use std::collections::{BTreeMap, BTreeSet};

pub mod resolver;
pub use resolver::resolve_css_rules;

/// 生成 `silex_macros/src/css/tw/resolver/table.rs` 与 `table_unimplement.rs` 代码
pub fn generate_macro_tables(
    classes: &[String],
    dynamic_prefixes: &BTreeMap<String, Vec<String>>,
) -> (String, String) {
    use std::fmt::Write;

    let candidate_set: BTreeSet<String> = classes.iter().cloned().collect();

    let mut static_entries: Vec<(String, Vec<(&'static str, String)>)> =
        Vec::with_capacity(candidate_set.len());
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

    // 1. 生成 table.rs
    let mut table_code = String::with_capacity(512 * 1024);
    table_code.push_str("// 自动生成的 Tailwind 静态规则表（供 silex_macros 使用）\n");
    table_code.push_str("// 避免手写硬编码，与 silex_codegen/resolver 保持 100% 规则对齐\n\n");
    table_code.push_str("use super::make_rule;\n");
    table_code.push_str("use crate::css::tw::ast::{Modifier, UtilityRule, UtilityValue};\n");
    table_code.push_str("use proc_macro2::Span;\n\n");

    table_code.push_str("#[allow(dead_code)]\n");
    table_code.push_str("#[derive(Clone, Copy)]\n");
    table_code.push_str("pub enum StaticVal {\n");
    table_code.push_str("    Kw(&'static str),\n");
    table_code.push_str("    Num(f64, &'static str),\n");
    table_code.push_str("    Hex(&'static str),\n");
    table_code.push_str("    Literal(&'static str),\n");
    table_code.push_str("    RingShadow,\n");
    table_code.push_str("}\n\n");

    table_code.push_str("#[rustfmt::skip]\n");
    table_code.push_str("pub const STATIC_CANDIDATE_UTILITIES: &[&str] = &[\n");
    for (class, _) in &static_entries {
        let _ = writeln!(table_code, "    \"{}\",", class);
    }
    table_code.push_str("];\n\n");

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

    table_code.push_str("#[rustfmt::skip]\n");
    table_code.push_str(
        "pub static STATIC_RULES: &[(&'static str, &'static [(&'static str, StaticVal)])] = &[\n",
    );
    for (class, rules) in &static_entries {
        let _ = writeln!(table_code, "    (\"{}\", &[", class);
        for (prop, val) in rules {
            let static_val_str = parse_val_to_static_val(val);
            let _ = writeln!(table_code, "        (\"{}\", {}),", prop, static_val_str);
        }
        table_code.push_str("    ]),\n");
    }
    table_code.push_str("];\n\n");

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
    table_unimplement_code.push_str("#[rustfmt::skip]\n");
    table_unimplement_code.push_str("pub const UNIMPLEMENTED_CANDIDATE_UTILITIES: &[&str] = &[\n");
    for class in &unimplemented_entries {
        let _ = writeln!(table_unimplement_code, "    \"{}\",", class);
    }
    table_unimplement_code.push_str("];\n");

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
