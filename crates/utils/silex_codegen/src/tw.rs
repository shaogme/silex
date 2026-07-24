use serde_json::Value;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

pub mod resolver;

pub use resolver::resolve_css_rules;

type RuleEntries = Vec<(String, Vec<(&'static str, Cow<'static, str>)>)>;

fn resolve_entries(classes: &[String]) -> (RuleEntries, Vec<String>) {
    let candidate_set: BTreeSet<String> = classes.iter().cloned().collect();

    let mut static_entries = Vec::with_capacity(candidate_set.len());
    let mut unimplemented_entries: Vec<String> = Vec::with_capacity(candidate_set.len() / 4);

    for class in candidate_set {
        if let Some(rules) = resolve_css_rules(&class) {
            static_entries.push((class, rules));
        } else {
            unimplemented_entries.push(class);
        }
    }

    static_entries.sort_by(|a, b| a.0.cmp(&b.0));
    unimplemented_entries.sort();

    (static_entries, unimplemented_entries)
}

fn push_table_header(code: &mut String, doc_comment: &str) {
    let _ = writeln!(code, "// {}", doc_comment);
    code.push_str("// 避免手写硬编码，与 silex_codegen/resolver 保持 100% 规则对齐\n\n");
    code.push_str("#[allow(unused_imports)]\nuse super::make_rule;\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::ast::{Modifier, UtilityRule, UtilityValue};\n");
    code.push_str("#[allow(unused_imports)]\nuse proc_macro2::Span;\n\n");

    code.push_str("#[allow(dead_code)]\n");
    code.push_str("#[derive(Clone, Copy)]\n");
    code.push_str("pub enum StaticVal {\n");
    code.push_str("    Kw(&'static str),\n");
    code.push_str("    Num(f64, &'static str),\n");
    code.push_str("    Hex(&'static str),\n");
    code.push_str("    Literal(&'static str),\n");
    code.push_str("    RingShadow,\n");
    code.push_str("}\n\n");
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

fn push_rules_array(code: &mut String, var_name: &str, entries: &RuleEntries) {
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(
        code,
        "pub static {}: &[(&'static str, &'static [(&'static str, StaticVal)])] = &[",
        var_name
    );
    for (class, rules) in entries {
        let _ = writeln!(code, "    (\"{}\", &[", class);
        for (prop, val) in rules {
            let static_val_str = parse_val_to_static_val(val);
            let _ = writeln!(code, "        (\"{}\", {}),", prop, static_val_str);
        }
        code.push_str("    ]),\n");
    }
    code.push_str("];\n\n");
}

/// 生成 `silex_macros/src/css/tw/resolver/table.rs` 与 `table_unimplement.rs` 代码
pub fn generate_macro_tables(
    classes: &[String],
    dynamic_prefixes: &BTreeMap<String, Vec<String>>,
) -> (String, String) {
    let (static_entries, unimplemented_entries) = resolve_entries(classes);

    // 1. 生成 table.rs
    let mut table_code = String::with_capacity(512 * 1024);
    push_table_header(
        &mut table_code,
        "自动生成的 Tailwind 静态规则表（供 silex_macros 使用）",
    );

    push_candidate_array(&mut table_code, "STATIC_CANDIDATE_UTILITIES", &static_entries);

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

    push_rules_array(&mut table_code, "STATIC_RULES", &static_entries);

    table_code.push_str(
        r#"pub fn resolve_static_rule(
    modifiers: &[Modifier],
    utility_token: &str,
    span: Span,
) -> Option<Vec<UtilityRule>> {
    let idx = STATIC_RULES.binary_search_by_key(&utility_token, |&(k, _)| k).ok()?;
    let entries = STATIC_RULES[idx].1;

    let mut rules = Vec::with_capacity(entries.len());
    for &(prop, val) in entries {
        let uval = match val {
            StaticVal::Kw(s) => UtilityValue::Keyword(s),
            StaticVal::Num(v, u) => UtilityValue::Numeric(v, u),
            StaticVal::Hex(s) => super::hex(s),
            StaticVal::Literal(s) => UtilityValue::ArbitraryLiteral(s.to_string()),
            StaticVal::RingShadow => UtilityValue::Keyword(super::RING_BOX_SHADOW),
        };
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

fn parse_val_to_static_val(val: &str) -> String {
    if val.contains("var(--tw-ring-inset") || val.contains("0 0 0 var(--tw-ring-offset-width") {
        return "StaticVal::RingShadow".to_string();
    }
    if val.starts_with('#') {
        return format!("StaticVal::Hex(\"{}\")", val);
    }
    if let Some((v, unit)) = try_parse_numeric(val) {
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
            return format!(
                "StaticVal::Kw(\"{}\")",
                val.replace('\\', "\\\\").replace('"', "\\\"")
            );
        }
        return format!(
            "StaticVal::Literal(\"{}\")",
            val.replace('\\', "\\\\").replace('"', "\\\"")
        );
    }
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
        if let Some(prefix) = val.strip_suffix(unit) {
            if let Ok(v) = prefix.parse::<f64>() {
                return Some((v, unit));
            }
        }
    }
    None
}

/// 生成 `table_examples.rs` 产物（生成方式与 `table.rs` 100% 一致，用于验证 test-cases 的生成与规则解析正确性）
pub fn generate_table_examples(test_cases: &[String]) -> String {
    let (static_entries, unimplemented_entries) = resolve_entries(test_cases);

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
    push_rules_array(&mut table_code, "TEST_CASE_RULES", &static_entries);

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
        ("border-x-width", &["border-left-width", "border-right-width"]),
        ("border-y-width", &["border-top-width", "border-bottom-width"]),
        ("border-x-style", &["border-left-style", "border-right-style"]),
        ("border-y-style", &["border-top-style", "border-bottom-style"]),
        ("border-x-color", &["border-left-color", "border-right-color"]),
        ("border-y-color", &["border-top-color", "border-bottom-color"]),
        ("scroll-margin-inline", &["scroll-margin-left", "scroll-margin-right"]),
        ("scroll-margin-block", &["scroll-margin-top", "scroll-margin-bottom"]),
        ("scroll-padding-inline", &["scroll-padding-left", "scroll-padding-right"]),
        ("scroll-padding-block", &["scroll-padding-top", "scroll-padding-bottom"]),
    ];

    for &(k, subs) in custom_aliases {
        raw_map.insert(k.to_string(), subs.iter().map(|s| s.to_string()).collect());
    }

    // 补充 border 顶层简写，确保能够全覆盖宽、样、色
    raw_map.entry("border".to_string()).or_insert_with(|| vec![
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
    ]);

    // 3. 多层简写递归解包为完全原子的 Longhand 属性
    let mut final_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (prop, _) in &raw_map {
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
    code.push_str("// 自动生成的 CSS / Tailwind 简写属性到原子子属性的静态对照表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动提取，切勿手动修改！\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static SHORTHAND_SUBPROPERTIES: &[(&'static str, &'static [&'static str])] = &[\n");
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
pub fn generate_prefix_metadata_code(
    prefix_metadata: &BTreeMap<String, PrefixMetaJson>,
) -> String {
    let mut code = String::with_capacity(16 * 1024);
    code.push_str("// 自动生成的 Utility 前缀与单位元数据表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("#[allow(dead_code)]\n");
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
    code.push_str("    pub prefix: &'static str,\n");
    code.push_str("    pub target_props: &'static [&'static str],\n");
    code.push_str("    pub unit_kind: UnitKind,\n");
    code.push_str("}\n\n");

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static PREFIX_METADATA: &[PrefixMeta] = &[\n");
    for (prefix, meta) in prefix_metadata {
        let _ = write!(code, "    PrefixMeta {{ prefix: \"{}\", target_props: &[", prefix);
        for (i, p) in meta.target_props.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "\"{}\"", p);
        }
        let _ = writeln!(code, "], unit_kind: UnitKind::{} }},", meta.unit_kind);
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据 Utility 前缀二分查找对应的元数据配置
pub fn lookup_prefix_meta(prefix: &str) -> Option<&'static PrefixMeta> {
    let idx = PREFIX_METADATA.binary_search_by_key(&prefix, |m| m.prefix).ok()?;
    Some(&PREFIX_METADATA[idx])
}
"#,
    );

    code
}

/// 生成 `silex_macros/src/css/tw/resolver/palette_gen.rs` 产物代码
pub fn generate_palette_code() -> String {
    let mut code = String::with_capacity(16 * 1024);
    code.push_str("// 自动生成的 Tailwind 标准色板表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    let raw_palette: &[(&str, [&str; 11])] = &[
        ("amber", ["#fffbeb", "#fef3c7", "#fde68a", "#fcd34d", "#fbbf24", "#f59e0b", "#d97706", "#b45309", "#92400e", "#78350f", "#451a03"]),
        ("blue", ["#eff6ff", "#dbeafe", "#bfdbfe", "#93c5fd", "#60a5fa", "#3b82f6", "#2563eb", "#1d4ed8", "#1e40af", "#1e3a8a", "#172554"]),
        ("cyan", ["#ecfeff", "#cffafe", "#a5f3fc", "#67e8f9", "#22d3ee", "#06b6d4", "#0891b2", "#0e7490", "#155e75", "#164e63", "#083344"]),
        ("emerald", ["#ecfdf5", "#d1fae5", "#a7f3d0", "#6ee7b7", "#34d399", "#10b981", "#059669", "#047857", "#065f46", "#064e3b", "#022c22"]),
        ("fuchsia", ["#fdf4ff", "#fae8ff", "#f5d0fe", "#f0abfc", "#e879f9", "#d946ef", "#c026d3", "#a21caf", "#86198f", "#701a75", "#4a044e"]),
        ("gray", ["#f9fafb", "#f3f4f6", "#e5e7eb", "#d1d5db", "#9ca3af", "#6b7280", "#4b5563", "#374151", "#1f2937", "#111827", "#030712"]),
        ("green", ["#f0fdf4", "#dcfce7", "#bbf7d0", "#86efac", "#4ade80", "#22c55e", "#16a34a", "#15803d", "#166534", "#14532d", "#052e16"]),
        ("indigo", ["#eef2ff", "#e0e7ff", "#c7d2fe", "#a5b4fc", "#818cf8", "#6366f1", "#4f46e5", "#4338ca", "#3730a3", "#312e81", "#1e1b4b"]),
        ("lime", ["#f7fee7", "#ecfccb", "#d9f99d", "#bef264", "#a3e635", "#84cc16", "#65a30d", "#4d7c0f", "#3f6212", "#365314", "#1a2e05"]),
        ("neutral", ["#fafafa", "#f5f5f5", "#e5e5e5", "#d4d4d4", "#a3a3a3", "#737373", "#525252", "#404040", "#262626", "#171717", "#0a0a0a"]),
        ("orange", ["#fff7ed", "#ffedd5", "#fed7aa", "#fdba74", "#fb923c", "#f97316", "#ea580c", "#c2410c", "#9a3412", "#7c2d12", "#431407"]),
        ("pink", ["#fdf2f8", "#fce7f3", "#fbcfe8", "#f9a8d4", "#f472b6", "#ec4899", "#db2777", "#be185d", "#9d174d", "#831843", "#500724"]),
        ("purple", ["#faf5ff", "#f3e8ff", "#e9d5ff", "#d8b4fe", "#c084fc", "#a855f7", "#9333ea", "#7e22ce", "#6b21a8", "#581c87", "#3b0764"]),
        ("red", ["#fef2f2", "#fee2e2", "#fecaca", "#fca5a5", "#f87171", "#ef4444", "#dc2626", "#b91c1c", "#991b1b", "#7f1d1d", "#450a0a"]),
        ("rose", ["#fff1f2", "#ffe4e6", "#fecdd3", "#fda4af", "#fb7185", "#f43f5e", "#e11d48", "#be123c", "#9f1239", "#881337", "#4c0519"]),
        ("sky", ["#f0f9ff", "#e0f2fe", "#bae6fd", "#7dd3fc", "#38bdf8", "#0ea5e9", "#0284c7", "#0369a1", "#075985", "#0c4a6e", "#082f49"]),
        ("slate", ["#f8fafc", "#f1f5f9", "#e2e8f0", "#cbd5e1", "#94a3b8", "#64748b", "#475569", "#334155", "#1e293b", "#0f172a", "#020617"]),
        ("stone", ["#fafaf9", "#f5f5f4", "#e7e5e4", "#d6d3d1", "#a8a29e", "#78716c", "#57534e", "#44403c", "#292524", "#1c1917", "#0c0a09"]),
        ("teal", ["#f0fdfa", "#ccfbf1", "#99f6e4", "#5eead4", "#2dd4bf", "#14b8a6", "#0d9488", "#0f766e", "#115e59", "#134e4a", "#042f2e"]),
        ("violet", ["#f5f3ff", "#ede9fe", "#ddd6fe", "#c4b5fd", "#a78bfa", "#8b5cf6", "#7c3aed", "#6d28d9", "#5b21b6", "#4c1d95", "#2e1065"]),
        ("yellow", ["#fefce8", "#fef9c3", "#fef08a", "#fde047", "#facc15", "#eab308", "#ca8a04", "#a16207", "#854d0e", "#713f12", "#422006"]),
        ("zinc", ["#fafafa", "#f4f4f5", "#e4e4e7", "#d4d4d8", "#a1a1aa", "#71717a", "#52525b", "#3f3f46", "#27272a", "#18181b", "#09090b"]),
    ];

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static PALETTE_TABLE: &[(&'static str, [&'static str; 11])] = &[\n");
    for &(name, shades) in raw_palette {
        let _ = write!(code, "    (\"{}\", [", name);
        for (i, hex) in shades.iter().enumerate() {
            if i > 0 {
                code.push_str(", ");
            }
            let _ = write!(code, "\"{}\"", hex);
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

    code
}

/// 生成 `silex_macros/src/css/tw/resolver/modifiers_gen.rs` 产物代码
pub fn generate_modifiers_code() -> String {
    let mut code = String::with_capacity(16 * 1024);
    code.push_str("// 自动生成的 Tailwind 修饰符与断点规则表（供 silex_macros 使用）\n");
    code.push_str("// 由 silex_codegen 自动生成，切勿手写修改！\n\n");

    code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\n");
    code.push_str("pub enum ModifierKind {\n");
    code.push_str("    PseudoClass,\n");
    code.push_str("    PseudoElement,\n");
    code.push_str("    MediaBreakpoint,\n");
    code.push_str("    Child,\n");
    code.push_str("    Descendant,\n");
    code.push_str("    Dark,\n");
    code.push_str("}\n\n");

    code.push_str("pub struct ModifierMeta {\n");
    code.push_str("    pub key: &'static str,\n");
    code.push_str("    pub kind: ModifierKind,\n");
    code.push_str("    pub priority: u32,\n");
    code.push_str("    pub css_selector: &'static str,\n");
    code.push_str("}\n\n");

    let raw_modifiers: &[(&str, &str, u32, &str)] = &[
        ("*", "ModifierKind::Child", 10, "& > *"),
        ("**", "ModifierKind::Descendant", 10, "& *"),
        ("2xl", "ModifierKind::MediaBreakpoint", 2536, "(min-width: 1536px)"),
        ("active", "ModifierKind::PseudoClass", 20, "&:active"),
        ("after", "ModifierKind::PseudoElement", 20, "&::after"),
        ("before", "ModifierKind::PseudoElement", 20, "&::before"),
        ("dark", "ModifierKind::Dark", 60, ".dark &, &.dark"),
        ("disabled", "ModifierKind::PseudoClass", 20, "&:disabled"),
        ("even", "ModifierKind::PseudoClass", 20, "&:nth-child(even)"),
        ("first", "ModifierKind::PseudoClass", 20, "&:first-child"),
        ("first-of-type", "ModifierKind::PseudoClass", 20, "&:first-of-type"),
        ("focus", "ModifierKind::PseudoClass", 20, "&:focus"),
        ("hover", "ModifierKind::PseudoClass", 20, "&:hover"),
        ("last", "ModifierKind::PseudoClass", 20, "&:last-child"),
        ("last-of-type", "ModifierKind::PseudoClass", 20, "&:last-of-type"),
        ("lg", "ModifierKind::MediaBreakpoint", 2024, "(min-width: 1024px)"),
        ("md", "ModifierKind::MediaBreakpoint", 1768, "(min-width: 768px)"),
        ("odd", "ModifierKind::PseudoClass", 20, "&:nth-child(odd)"),
        ("only", "ModifierKind::PseudoClass", 20, "&:only-child"),
        ("only-of-type", "ModifierKind::PseudoClass", 20, "&:only-of-type"),
        ("placeholder", "ModifierKind::PseudoElement", 20, "&::placeholder"),
        ("sm", "ModifierKind::MediaBreakpoint", 1640, "(min-width: 640px)"),
        ("visited", "ModifierKind::PseudoClass", 20, "&:visited"),
        ("xl", "ModifierKind::MediaBreakpoint", 2280, "(min-width: 1280px)"),
    ];

    code.push_str("#[rustfmt::skip]\n");
    code.push_str("pub static MODIFIER_TABLE: &[ModifierMeta] = &[\n");
    for &(key, kind, p, sel) in raw_modifiers {
        let _ = writeln!(
            code,
            "    ModifierMeta {{ key: \"{}\", kind: {}, priority: {}, css_selector: \"{}\" }},",
            key, kind, p, sel
        );
    }
    code.push_str("];\n\n");

    code.push_str(
        r#"/// 根据修饰符 key 二分查找对应的元数据配置
pub fn lookup_modifier_meta(key: &str) -> Option<&'static ModifierMeta> {
    let idx = MODIFIER_TABLE.binary_search_by_key(&key, |m| m.key).ok()?;
    Some(&MODIFIER_TABLE[idx])
}
"#,
    );

    code
}

/// 生成 `silex_macros/src/css/tw/resolver/keyframes_gen.rs` 产物代码
pub fn generate_keyframes_code() -> String {
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
    code.push_str("    KeyframeMeta {\n");
    code.push_str("        name: \"bounce\",\n");
    code.push_str("        steps: &[\n");
    code.push_str("            KeyframeStep { selector: \"0%, 100%\", declarations: &[(\"transform\", \"translateY(-25%)\"), (\"animation-timing-function\", \"cubic-bezier(0.8, 0, 1, 1)\")] },\n");
    code.push_str("            KeyframeStep { selector: \"50%\", declarations: &[(\"transform\", \"none\"), (\"animation-timing-function\", \"cubic-bezier(0, 0, 0.2, 1)\")] },\n");
    code.push_str("        ],\n");
    code.push_str("    },\n");
    code.push_str("    KeyframeMeta {\n");
    code.push_str("        name: \"ping\",\n");
    code.push_str("        steps: &[\n");
    code.push_str("            KeyframeStep { selector: \"75%, 100%\", declarations: &[(\"transform\", \"scale(2)\"), (\"opacity\", \"0\")] },\n");
    code.push_str("        ],\n");
    code.push_str("    },\n");
    code.push_str("    KeyframeMeta {\n");
    code.push_str("        name: \"pulse\",\n");
    code.push_str("        steps: &[\n");
    code.push_str("            KeyframeStep { selector: \"50%\", declarations: &[(\"opacity\", \"0.5\")] },\n");
    code.push_str("        ],\n");
    code.push_str("    },\n");
    code.push_str("    KeyframeMeta {\n");
    code.push_str("        name: \"spin\",\n");
    code.push_str("        steps: &[\n");
    code.push_str("            KeyframeStep { selector: \"from\", declarations: &[(\"transform\", \"rotate(0deg)\")] },\n");
    code.push_str("            KeyframeStep { selector: \"to\", declarations: &[(\"transform\", \"rotate(360deg)\")] },\n");
    code.push_str("        ],\n");
    code.push_str("    },\n");
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




