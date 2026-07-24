use serde_json::Value;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

pub mod resolver;

pub use resolver::resolve_css_rules;

type RuleEntries<'a> = Vec<(String, Vec<(&'static str, Cow<'a, str>)>)>;

fn resolve_entries<'a>(
    classes: &[String],
    palette: &'a BTreeMap<String, Vec<ColorShadeInfo>>,
) -> (RuleEntries<'a>, Vec<String>) {
    let candidate_set: BTreeSet<String> = classes.iter().cloned().collect();

    let mut static_entries = Vec::with_capacity(candidate_set.len());
    let mut unimplemented_entries: Vec<String> = Vec::with_capacity(candidate_set.len() / 4);

    for class in candidate_set {
        if let Some(rules) = resolve_css_rules(&class, palette) {
            static_entries.push((class, rules));
        } else {
            unimplemented_entries.push(class);
        }
    }

    static_entries.sort_by(|a, b| a.0.cmp(&b.0));
    unimplemented_entries.sort();

    (static_entries, unimplemented_entries)
}

#[derive(Default, Debug)]
struct UsedStaticValKinds {
    kw: bool,
    num: bool,
    hex: bool,
    literal: bool,
    ring_shadow: bool,
}

fn push_table_header(code: &mut String, doc_comment: &str) {
    let _ = writeln!(code, "// {}", doc_comment);
    code.push_str("// 避免手写硬编码，与 silex_codegen/resolver 保持 100% 规则对齐\n\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::ast::{Modifier, SpannedModifier, UtilityRule, UtilityValue};\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::resolver::make_rule;\n");
    code.push_str("#[allow(unused_imports)]\nuse proc_macro2::Span;\n\n");
}

fn push_candidate_array(code: &mut String, var_name: &str, entries: &RuleEntries) {
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(code, "pub const {}: &[&str] = &[", var_name);
    for (class, _) in entries {
        let _ = writeln!(code, "    \"{}\",", class);
    }
    code.push_str("];\n\n");
}

fn push_unimplemented_array(code: &mut String, var_name: &str, entries: &[String]) {
    if entries.is_empty() {
        return;
    }
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(code, "pub const {}: &[&str] = &[", var_name);
    for class in entries {
        let _ = writeln!(code, "    \"{}\",", class);
    }
    code.push_str("];\n\n");
}

fn push_rules_array(
    code: &mut String,
    var_name: &str,
    entries: &RuleEntries,
) -> UsedStaticValKinds {
    let mut used = UsedStaticValKinds::default();
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(
        code,
        "pub static {}: &[(&str, &[(&str, StaticVal)])] = &[",
        var_name
    );
    for (class, rules) in entries {
        let _ = writeln!(code, "    (\"{}\", &[", class);
        for (prop, val) in rules {
            let static_val_str = parse_val_to_static_val(val, &mut used);
            let _ = writeln!(code, "        (\"{}\", {}),", prop, static_val_str);
        }
        code.push_str("    ]),\n");
    }
    code.push_str("];\n\n");
    used
}

fn push_static_val_enum(code: &mut String, used: &UsedStaticValKinds) {
    code.push_str("#[derive(Clone, Copy)]\n");
    code.push_str("pub enum StaticVal {\n");
    if used.kw {
        code.push_str("    Kw(&'static str),\n");
    }
    if used.num {
        code.push_str("    Num(f64, &'static str),\n");
    }
    if used.hex {
        code.push_str("    Hex(&'static str),\n");
    }
    if used.literal {
        code.push_str("    Literal(&'static str),\n");
    }
    if used.ring_shadow {
        code.push_str("    RingShadow,\n");
    }
    code.push_str("}\n\n");
}

fn push_resolve_static_rule_fn(code: &mut String, rules_var_name: &str, used: &UsedStaticValKinds) {
    let _ = writeln!(
        code,
        r#"pub fn resolve_static_rule(
    modifiers: &[SpannedModifier],
    utility_token: &str,
    span: Span,
) -> Option<Vec<UtilityRule>> {{
    let idx = {rules_var_name}.binary_search_by_key(&utility_token, |&(k, _)| k).ok()?;
    let entries = {rules_var_name}[idx].1;

    let mut rules = Vec::with_capacity(entries.len());
    for &(prop, val) in entries {{
        let uval = match val {{"#
    );

    if used.kw {
        code.push_str("            StaticVal::Kw(s) => UtilityValue::Keyword(s),\n");
    }
    if used.num {
        code.push_str("            StaticVal::Num(v, u) => UtilityValue::Numeric(v, u),\n");
    }
    if used.hex {
        code.push_str("            StaticVal::Hex(s) => crate::css::tw::resolver::hex(s),\n");
    }
    if used.literal {
        code.push_str(
            "            StaticVal::Literal(s) => UtilityValue::ArbitraryLiteral(s.to_string()),\n",
        );
    }
    if used.ring_shadow {
        code.push_str("            StaticVal::RingShadow => UtilityValue::Keyword(crate::css::tw::resolver::RING_BOX_SHADOW),\n");
    }

    code.push_str(
        r#"        };
        rules.push(make_rule(
            if modifiers.is_empty() { Vec::new() } else { modifiers.to_vec() },
            prop,
            uval,
            span,
        ));
    }
    Some(rules)
}
"#,
    );
}

/// 生成 `silex_macros/src/css/tw/resolver/codegen/table.rs` 与 `table_unimplement.rs` 代码
pub fn generate_macro_tables(
    classes: &[String],
    dynamic_prefixes: &BTreeMap<String, Vec<String>>,
    palette: &BTreeMap<String, Vec<ColorShadeInfo>>,
) -> (String, String) {
    let (static_entries, unimplemented_entries) = resolve_entries(classes, palette);

    // 1. 生成 table.rs
    let mut table_code = String::with_capacity(512 * 1024);
    push_table_header(
        &mut table_code,
        "自动生成的 Tailwind 静态规则表（供 silex_macros 使用）",
    );

    push_candidate_array(
        &mut table_code,
        "STATIC_CANDIDATE_UTILITIES",
        &static_entries,
    );

    table_code.push_str("#[rustfmt::skip]\n");
    table_code.push_str("pub const DYNAMIC_UTILITY_PREFIXES: &[(&str, &[&str])] = &[\n");
    for (prefix, suffixes) in dynamic_prefixes {
        let _ = write!(table_code, "    (\"{}\", &[", prefix);
        for (i, suffix) in suffixes.iter().enumerate() {
            if i > 0 {
                table_code.push_str(", ");
            }
            let _ = write!(table_code, "\"{}\"", suffix);
        }
        table_code.push_str("]),\n");
    }
    table_code.push_str("];\n\n");

    let used = push_rules_array(&mut table_code, "STATIC_RULES", &static_entries);
    push_static_val_enum(&mut table_code, &used);
    push_resolve_static_rule_fn(&mut table_code, "STATIC_RULES", &used);

    // 2. 生成 table_unimplement.rs
    let mut table_unimplement_code = String::with_capacity(128 * 1024);
    table_unimplement_code
        .push_str("// 自动生成的 Tailwind 未匹配/未实现静态类名表（供 silex_macros 使用）\n");
    table_unimplement_code.push_str("// 记录已知但当前尚未在 resolver 中实现 CSS 规则的类名\n\n");
    push_unimplemented_array(
        &mut table_unimplement_code,
        "UNIMPLEMENTED_CANDIDATE_UTILITIES",
        &unimplemented_entries,
    );

    (table_code, table_unimplement_code)
}

fn parse_val_to_static_val(val: &str, used: &mut UsedStaticValKinds) -> String {
    if val.contains("var(--tw-ring-inset") || val.contains("0 0 0 var(--tw-ring-offset-width") {
        used.ring_shadow = true;
        return "StaticVal::RingShadow".to_string();
    }
    if val.starts_with('#') {
        used.hex = true;
        return format!("StaticVal::Hex(\"{}\")", val);
    }
    if let Some((v, unit)) = try_parse_numeric(val) {
        used.num = true;
        return format!("StaticVal::Num({:?}, \"{}\")", v, unit);
    }
    if val.contains('(') || val.contains(' ') || val.contains('/') || val.contains(',') {
        if val.starts_with("linear-gradient(")
            || val.starts_with("radial-gradient(")
            || val.starts_with("conic-gradient(")
            || val.starts_with("calc(")
            || val.starts_with("rotate(")
            || val.starts_with("translateX(")
            || val.starts_with("translateY(")
            || val.starts_with("blur(")
            || val.starts_with("minmax(")
            || val.starts_with("repeat(")
        {
            used.kw = true;
            return format!(
                "StaticVal::Kw(\"{}\")",
                val.replace('\\', "\\\\").replace('"', "\\\"")
            );
        }
        used.literal = true;
        return format!(
            "StaticVal::Literal(\"{}\")",
            val.replace('\\', "\\\\").replace('"', "\\\"")
        );
    }
    used.kw = true;
    format!(
        "StaticVal::Kw(\"{}\")",
        val.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

fn try_parse_numeric(val: &str) -> Option<(f64, &'static str)> {
    if let Ok(v) = val.parse::<f64>() {
        return Some((v, ""));
    }
    let units = ["rem", "px", "%", "vw", "vh", "em", "deg", "ms", "s"];
    for &unit in &units {
        if let Some(prefix) = val.strip_suffix(unit)
            && let Ok(v) = prefix.parse::<f64>()
        {
            return Some((v, unit));
        }
    }
    None
}

/// 生成 `table_examples.rs` 产物（生成方式与 `table.rs` 100% 一致，用于验证 test-cases 的生成与规则解析正确性）
pub fn generate_table_examples(
    test_cases: &[String],
    palette: &BTreeMap<String, Vec<ColorShadeInfo>>,
) -> String {
    let (static_entries, unimplemented_entries) = resolve_entries(test_cases, palette);

    let mut table_code = String::with_capacity(64 * 1024);
    push_table_header(
        &mut table_code,
        "自动生成的 Tailwind 测试用例规则表（用于验证 test-cases 的生成与 CSS 规则解析正确性）\n// 对应 tailwind-classes.json 中的 test_cases",
    );

    push_candidate_array(
        &mut table_code,
        "TEST_CASE_CANDIDATE_UTILITIES",
        &static_entries,
    );
    push_unimplemented_array(
        &mut table_code,
        "UNIMPLEMENTED_TEST_CASE_UTILITIES",
        &unimplemented_entries,
    );
    let used = push_rules_array(&mut table_code, "TEST_CASE_RULES", &static_entries);
    push_static_val_enum(&mut table_code, &used);

    table_code
}

/// 生成 `silex_macros/src/css/tw/resolver/shorthands.rs` 核心代码
pub fn generate_shorthands_code(props_json_str: &str) -> String {
    let mut raw_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // 1. 从 MDN JSON 中提取简写/子属性列表
    if let Ok(json_map) = serde_json::from_str::<BTreeMap<String, Value>>(props_json_str) {
        for (prop_name, prop_val) in json_map {
            if prop_name.starts_with('-') || prop_name.starts_with("--") {
                continue;
            }
            if let Some(comp_arr) = prop_val.get("computed").and_then(|v| v.as_array()) {
                let subs: Vec<String> = comp_arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|s| s != &prop_name)
                    .collect();
                if !subs.is_empty() {
                    raw_map.insert(prop_name.clone(), subs);
                }
            }
        }
    }

    // 2. 注入 Tailwind 专属 / CSS 扩展简写别名
    let custom_aliases: &[(&str, &[&str])] = &[
        ("padding-inline", &["padding-left", "padding-right"]),
        ("padding-block", &["padding-top", "padding-bottom"]),
        ("margin-inline", &["margin-left", "margin-right"]),
        ("margin-block", &["margin-top", "margin-bottom"]),
        ("inset", &["top", "right", "bottom", "left"]),
        ("inset-x", &["left", "right"]),
        ("inset-y", &["top", "bottom"]),
        ("border-x", &["border-left-width", "border-right-width"]),
        ("border-y", &["border-top-width", "border-bottom-width"]),
        (
            "border-x-width",
            &["border-left-width", "border-right-width"],
        ),
        (
            "border-y-width",
            &["border-top-width", "border-bottom-width"],
        ),
        (
            "border-x-style",
            &["border-left-style", "border-right-style"],
        ),
        (
            "border-y-style",
            &["border-top-style", "border-bottom-style"],
        ),
        (
            "border-x-color",
            &["border-left-color", "border-right-color"],
        ),
        (
            "border-y-color",
            &["border-top-color", "border-bottom-color"],
        ),
        (
            "scroll-margin-inline",
            &["scroll-margin-left", "scroll-margin-right"],
        ),
        (
            "scroll-margin-block",
            &["scroll-margin-top", "scroll-margin-bottom"],
        ),
        (
            "scroll-padding-inline",
            &["scroll-padding-left", "scroll-padding-right"],
        ),
        (
            "scroll-padding-block",
            &["scroll-padding-top", "scroll-padding-bottom"],
        ),
    ];

    for &(k, subs) in custom_aliases {
        raw_map.insert(k.to_string(), subs.iter().map(|s| s.to_string()).collect());
    }

    // 补充 border 顶层简写，确保能够全覆盖宽、样、色
    raw_map.entry("border".to_string()).or_insert_with(|| {
        vec![
            "border-top-width".to_string(),
            "border-right-width".to_string(),
            "border-bottom-width".to_string(),
            "border-left-width".to_string(),
            "border-top-style".to_string(),
            "border-right-style".to_string(),
            "border-bottom-style".to_string(),
            "border-left-style".to_string(),
            "border-top-color".to_string(),
            "border-right-color".to_string(),
            "border-bottom-color".to_string(),
            "border-left-color".to_string(),
        ]
    });

    // 3. 多层简写递归解包为完全原子的 Longhand 属性
    let mut final_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for prop in raw_map.keys() {
        let mut atomic_set = BTreeSet::new();
        flatten_prop(prop, &raw_map, &mut atomic_set, &mut BTreeSet::new());
        if !atomic_set.is_empty() {
            let mut list: Vec<String> = atomic_set.into_iter().collect();
            list.sort();
            final_map.insert(prop.clone(), list);
        }
    }

    // 4. 生成 Rust 代码
    let mut code = String::with_capacity(32 * 1024);
    code.push_str(
        "// 自动生成的 CSS / Tailwind 简写属性到原子子属性的静态对照表（供 silex_macros 使用）\n",
    );
    code.push_str("// 由 silex_codegen 自动提取，切勿手动修改！\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static SHORTHAND_SUBPROPERTIES: &[(&str, &[&str])] = &[\n");
    for (k, subs) in &final_map {
        let _ = write!(code, "    (\"{}\", &[", k);
        for (i, sub) in subs.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "\"{}\"", sub);
        }
        code.push_str("]),\n");
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 获取指定 CSS / Tailwind 简写属性拆解后的原子子属性集合
pub fn get_atomic_subproperties(prop: &str) -> Option<&'static [&'static str]> {
    let idx = SHORTHAND_SUBPROPERTIES.binary_search_by_key(&prop, |&(k, _)| k).ok()?;
    Some(SHORTHAND_SUBPROPERTIES[idx].1)
}
"#,
    );

    code
}

fn flatten_prop(
    prop: &str,
    raw_map: &BTreeMap<String, Vec<String>>,
    out: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) {
    if !visited.insert(prop.to_string()) {
        return;
    }
    if let Some(subs) = raw_map.get(prop) {
        for sub in subs {
            if raw_map.contains_key(sub) {
                flatten_prop(sub, raw_map, out, visited);
            } else {
                out.insert(sub.clone());
            }
        }
    } else {
        out.insert(prop.to_string());
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct PrefixMetaJson {
    pub target_props: Vec<String>,
    pub unit_kind: String,
}

/// 生成 `silex_macros/src/css/tw/resolver/prefix_metadata.rs` 产物代码
pub fn generate_prefix_metadata_code(prefix_metadata: &BTreeMap<String, PrefixMetaJson>) -> String {
    let mut code = String::with_capacity(16 * 1024);
    code.push_str("// 自动生成的 Utility 前缀与单位元数据表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    code.push_str("pub enum UnitKind {\n");
    code.push_str("    RemScale,\n");
    code.push_str("    Pixel,\n");
    code.push_str("    Percentage,\n");
    code.push_str("    Degree,\n");
    code.push_str("    Milliseconds,\n");
    code.push_str("    Unitless,\n");
    code.push_str("    GridRepeat,\n");
    code.push_str("    GridSpan,\n");
    code.push_str("}\n\n");

    code.push_str("pub struct PrefixMeta {\n");
    code.push_str("    pub target_props: &'static [&'static str],\n");
    code.push_str("    pub unit_kind: UnitKind,\n");
    code.push_str("}\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static PREFIX_METADATA: &[PrefixMeta] = &[\n");
    for meta in prefix_metadata.values() {
        let _ = write!(code, "    PrefixMeta {{ target_props: &[");
        for (i, p) in meta.target_props.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "\"{}\"", p);
        }
        let _ = writeln!(code, "], unit_kind: UnitKind::{} }},", meta.unit_kind);
    }
    code.push_str("];\n\n");

    code.push_str("/// 根据 Utility 前缀静态匹配对应的元数据配置\n");
    code.push_str("pub fn lookup_prefix_meta(prefix: &str) -> Option<&'static PrefixMeta> {\n");
    code.push_str("    match prefix {\n");
    for (i, (prefix, _)) in prefix_metadata.iter().enumerate() {
        let _ = writeln!(
            code,
            "        \"{}\" => Some(&PREFIX_METADATA[{}]),",
            prefix, i
        );
    }
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    code
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ColorShadeInfo {
    pub shade: String,
    pub hex: String,
    pub raw: String,
    pub rgb: [u8; 3],
}

/// 生成 `silex_macros/src/css/tw/resolver/palette_gen.rs` 产物代码
pub fn generate_palette_code(palette: &BTreeMap<String, Vec<ColorShadeInfo>>) -> String {
    let mut code = String::with_capacity(32 * 1024);
    code.push_str("// 自动生成的 Tailwind 标准色板表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static PALETTE_TABLE: &[(&str, [&str; 11])] = &[\n");
    for (name, shades) in palette {
        let _ = write!(code, "    (\"{}\", [", name);
        for (i, info) in shades.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "\"{}\"", info.hex);
        }
        code.push_str("]),\n");
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据色系名称获取标准的 11 阶梯 Hex 阵列
pub fn get_raw_palette(color_name: &str) -> Option<[&'static str; 11]> {
    let idx = PALETTE_TABLE.binary_search_by_key(&color_name, |&(k, _)| k).ok()?;
    Some(PALETTE_TABLE[idx].1)
}

"#,
    );

    code.push_str("/// 编译期生成的 O(1) 静态色板 Hex 匹配器\n");
    code.push_str("pub fn lookup_palette_color_fast(color_name: &str, shade: &str) -> Option<&'static str> {\n");
    code.push_str("    match (color_name, shade) {\n");
    for (name, shades) in palette {
        for info in shades {
            let _ = writeln!(
                code,
                "        (\"{}\", \"{}\") => Some(\"{}\"),",
                name, info.shade, info.hex
            );
        }
    }
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    let standard_opacities: &[u32] = &[
        0, 5, 10, 15, 20, 25, 30, 40, 50, 60, 70, 75, 80, 90, 95, 100,
    ];
    code.push_str("/// 编译期预计算的标准 Alpha RGBA 静态匹配器 (消除运行时 hex_to_rgba 格式化)\n");
    code.push_str("pub fn lookup_palette_rgba_fast(color_name: &str, shade: &str, opacity: u32) -> Option<&'static str> {\n");
    code.push_str("    match (color_name, shade, opacity) {\n");
    for (name, shades) in palette {
        for info in shades {
            let [r, g, b] = info.rgb;
            for &op in standard_opacities {
                let alpha = op as f64 / 100.0;
                let alpha_str = if op % 10 == 0 || op % 25 == 0 || op == 5 || op == 15 || op == 95 {
                    format!("{:.2}", alpha)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string()
                } else {
                    format!("{:.3}", alpha)
                };
                let rgba_str = format!("rgba({}, {}, {}, {})", r, g, b, alpha_str);
                let _ = writeln!(
                    code,
                    "        (\"{}\", \"{}\", {}) => Some(\"{}\"),",
                    name, info.shade, op, rgba_str
                );
            }
        }
    }
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    code
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct ModifierMetaJson {
    pub key: String,
    pub kind: String,
    pub priority: u32,
    pub css_selector: String,
}

/// 生成 `silex_macros/src/css/tw/resolver/modifiers_gen.rs` 产物代码
pub fn generate_modifiers_code(modifiers: &[ModifierMetaJson]) -> String {
    let mut code = String::with_capacity(32 * 1024);
    code.push_str("// 自动生成的 Tailwind 修饰符与断点规则表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");
    code.push_str("use crate::css::tw::ast::Modifier;\n\n");

    code.push_str("pub struct ModifierMeta {\n");
    code.push_str("    pub key: &'static str,\n");
    code.push_str("    pub priority: u32,\n");
    code.push_str("    pub css_selector: &'static str,\n");
    code.push_str("}\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static MODIFIER_TABLE: &[ModifierMeta] = &[\n");
    for meta in modifiers {
        let _ = writeln!(
            code,
            "    ModifierMeta {{ key: \"{}\", priority: {}, css_selector: \"{}\" }},",
            meta.key,
            meta.priority,
            meta.css_selector.replace('\\', "\\\\").replace('"', "\\\"")
        );
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据修饰符 key 二分查找对应的元数据配置
pub fn lookup_modifier_meta(key: &str) -> Option<&'static ModifierMeta> {
    let idx = MODIFIER_TABLE.binary_search_by_key(&key, |m| m.key).ok()?;
    Some(&MODIFIER_TABLE[idx])
}

fn split_state_and_name_fast(rest: &str) -> (String, Option<String>) {
    if let Some(slash_idx) = rest.rfind('/') {
        let name_part = &rest[slash_idx + 1..];
        let state_part = &rest[..slash_idx];
        if !name_part.is_empty()
            && name_part
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            let open_brackets = state_part.chars().filter(|&c| c == '[').count();
            let close_brackets = state_part.chars().filter(|&c| c == ']').count();
            if open_brackets == close_brackets {
                return (state_part.to_string(), Some(name_part.to_string()));
            }
        }
    }
    (rest.to_string(), None)
}

fn parse_bracket_kv_fast(rest: &str) -> (String, Option<String>) {
    if rest.starts_with('[') && rest.ends_with(']') {
        let inner = &rest[1..rest.len() - 1];
        if let Some((k, v)) = inner.split_once('=') {
            (k.to_string(), Some(v.to_string()))
        } else {
            (inner.to_string(), None)
        }
    } else {
        (rest.to_string(), None)
    }
}

fn parse_container_query_fast(container_spec: &str) -> Modifier {
    let (c_name, spec) = if let Some((name, rest)) = container_spec.split_once('/') {
        (Some(name.to_string()), rest)
    } else {
        (None, container_spec)
    };

    let min_width = match spec {
        "sm" => "640px".to_string(),
        "md" => "768px".to_string(),
        "lg" => "1024px".to_string(),
        "xl" => "1280px".to_string(),
        "2xl" => "1536px".to_string(),
        _ => {
            let cleaned = spec.strip_prefix("min-").unwrap_or(spec);
            let cleaned = cleaned.strip_prefix('-').unwrap_or(cleaned);
            if cleaned.starts_with('[') && cleaned.ends_with(']') {
                cleaned[1..cleaned.len() - 1].to_string()
            } else {
                cleaned.to_string()
            }
        }
    };

    Modifier::ContainerQuery {
        name: c_name,
        min_width,
    }
}

/// 编译期生成的 Modifier 状态机/快速匹配器
pub fn parse_modifier_fast(prefix: &str) -> Option<Modifier> {
    // 1. 编译期静态 match 确切 Modifier (比二分查找更快的 Match DFA)
    match prefix {
"#,
    );

    for meta in modifiers {
        let arm_expr = match meta.kind.as_str() {
            "Child" => "Modifier::Child".to_string(),
            "Descendant" => "Modifier::Descendant".to_string(),
            "Dark" => "Modifier::Dark".to_string(),
            "MediaBreakpoint" => format!("Modifier::MediaBreakpoint(\"{}\".to_string())", meta.key),
            "PseudoClass" => format!("Modifier::PseudoClass(\"{}\".to_string())", meta.key),
            "PseudoElement" => format!("Modifier::PseudoElement(\"{}\".to_string())", meta.key),
            _ => continue,
        };
        let _ = writeln!(
            code,
            "        \"{}\" => return Some({}),",
            meta.key, arm_expr
        );
    }

    code.push_str(
        r#"        _ => {}
    }

    // 2. 前缀状态匹配 (Prefix Dispatcher)
    if let Some(spec) = prefix.strip_prefix('@') {
        return Some(parse_container_query_fast(spec));
    }

    if let Some(rest) = prefix.strip_prefix("group-") {
        let (state, name) = split_state_and_name_fast(rest);
        return Some(Modifier::Group { state, name });
    }

    if let Some(rest) = prefix.strip_prefix("peer-") {
        let (state, name) = split_state_and_name_fast(rest);
        return Some(Modifier::Peer { state, name });
    }

    if let Some(rest) = prefix.strip_prefix("data-") {
        let (key, value) = parse_bracket_kv_fast(rest);
        return Some(Modifier::DataAttribute { key, value });
    }

    if let Some(rest) = prefix.strip_prefix("aria-") {
        let (key, value) = parse_bracket_kv_fast(rest);
        let value = value.or_else(|| Some("true".to_string()));
        return Some(Modifier::AriaAttribute { key, value });
    }

    if let Some(rest) = prefix.strip_prefix("has-") {
        return Some(Modifier::Has(format!("has-{}", rest)));
    }

    if prefix.starts_with('[') && prefix.ends_with(']') {
        return Some(Modifier::CustomSelector(prefix[1..prefix.len() - 1].to_string()));
    }

    None
}
"#,
    );

    code
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct KeyframeStepJson {
    pub selector: String,
    pub declarations: Vec<(String, String)>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct KeyframeMetaJson {
    pub name: String,
    pub steps: Vec<KeyframeStepJson>,
}

/// 生成 `silex_macros/src/css/tw/resolver/keyframes_gen.rs` 产物代码
pub fn generate_keyframes_code(keyframes: &[KeyframeMetaJson]) -> String {
    let mut code = String::with_capacity(16 * 1024);
    code.push_str("// 自动生成的动画 Keyframes 规则表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("pub struct KeyframeStep {\n");
    code.push_str("    pub selector: &'static str,\n");
    code.push_str("    pub declarations: &'static [(&'static str, &'static str)],\n");
    code.push_str("}\n\n");

    code.push_str("pub struct KeyframeMeta {\n");
    code.push_str("    pub name: &'static str,\n");
    code.push_str("    pub steps: &'static [KeyframeStep],\n");
    code.push_str("}\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static KEYFRAME_TABLE: &[KeyframeMeta] = &[\n");
    for meta in keyframes {
        let _ = writeln!(code, "    KeyframeMeta {{");
        let _ = writeln!(
            code,
            "        name: \"{}\",",
            meta.name.replace('\\', "\\\\").replace('"', "\\\"")
        );
        code.push_str("        steps: &[\n");
        for step in &meta.steps {
            let decls_str = step
                .declarations
                .iter()
                .map(|(p, v)| {
                    format!(
                        "(\"{}\", \"{}\")",
                        p.replace('\\', "\\\\").replace('"', "\\\""),
                        v.replace('\\', "\\\\").replace('"', "\\\"")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                code,
                "            KeyframeStep {{ selector: \"{}\", declarations: &[{}] }},",
                step.selector.replace('\\', "\\\\").replace('"', "\\\""),
                decls_str
            );
        }
        code.push_str("        ],\n");
        code.push_str("    },\n");
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据动画 keyframe 名称二分查找关键帧元数据配置
pub fn lookup_keyframe_meta(name: &str) -> Option<&'static KeyframeMeta> {
    let idx = KEYFRAME_TABLE.binary_search_by_key(&name, |k| k.name).ok()?;
    Some(&KEYFRAME_TABLE[idx])
}
"#,
    );

    code
}
