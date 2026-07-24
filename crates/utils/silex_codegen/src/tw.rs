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




