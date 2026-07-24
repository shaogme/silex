use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use super::{palette::ColorShadeInfo, property_id::to_pascal_case, resolver::resolve_css_rules};

pub type RuleEntries<'a> = Vec<(String, Vec<(&'static str, Cow<'a, str>)>)>;

pub fn resolve_entries<'a>(
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
pub struct UsedStaticValKinds {
    pub kw: bool,
    pub num: bool,
    pub hex: bool,
    pub literal: bool,
    pub ring_shadow: bool,
}

pub fn push_table_header(code: &mut String, doc_comment: &str) {
    let _ = writeln!(code, "// {}", doc_comment);
    code.push_str("// 避免手写硬编码，与 silex_codegen/resolver 保持 100% 规则对齐\n\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::ast::{Modifier, SpannedModifier, UtilityRule, UtilityValue};\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::resolver::codegen::property_id::CssPropertyId;\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::resolver::make_rule;\n");
    code.push_str("#[allow(unused_imports)]\nuse proc_macro2::Span;\n\n");
}

pub fn push_candidate_array(code: &mut String, var_name: &str, entries: &RuleEntries) {
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(code, "pub const {}: &[&str] = &[", var_name);
    for (class, _) in entries {
        let _ = writeln!(code, "    \"{}\",", class);
    }
    code.push_str("];\n\n");
}

pub fn push_unimplemented_array(code: &mut String, var_name: &str, entries: &[String]) {
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

pub fn push_rules_array(
    code: &mut String,
    var_name: &str,
    entries: &RuleEntries,
) -> UsedStaticValKinds {
    let mut used = UsedStaticValKinds::default();
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(
        code,
        "pub static {}: &[(&str, &[(CssPropertyId, StaticVal)])] = &[",
        var_name
    );
    for (class, rules) in entries {
        let _ = writeln!(code, "    (\"{}\", &[", class);
        for (prop, val) in rules {
            let static_val_str = parse_val_to_static_val(val, &mut used);
            let prop_variant = to_pascal_case(prop);
            let _ = writeln!(code, "        (CssPropertyId::{}, {}),", prop_variant, static_val_str);
        }
        code.push_str("    ]),\n");
    }
    code.push_str("];\n\n");
    used
}

pub fn push_static_val_enum(code: &mut String, used: &UsedStaticValKinds) {
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

pub fn push_resolve_static_rule_fn(code: &mut String, rules_var_name: &str, used: &UsedStaticValKinds) {
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
            if modifiers.is_empty() { smallvec::SmallVec::new() } else { modifiers.iter().cloned().collect() },
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

pub fn parse_val_to_static_val(val: &str, used: &mut UsedStaticValKinds) -> String {
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

pub fn try_parse_numeric(val: &str) -> Option<(f64, &'static str)> {
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
