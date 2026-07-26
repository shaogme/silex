use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use silex_tw_core::{ColorShadeInfo, JsonPalette, TwRuleSet, TwValueKind, classify, resolve_class};

use super::property_id::to_pascal_case;

/// 一条静态表记录：类名 + 伴生选择器 + 声明列表
pub struct RuleEntry {
    pub class: String,
    pub selector: Option<&'static str>,
    pub decls: Vec<(&'static str, String)>,
}

pub type RuleEntries = Vec<RuleEntry>;

/// 把 core 的解析结果压成一条静态表记录。
///
/// 静态表的每一行只能挂一个选择器；真出现一个类名同时产出多个选择器分组，
/// 说明表结构需要升级，此时直接让构建失败，而不是悄悄丢掉其中一组。
fn flatten(class: &str, sets: Vec<TwRuleSet>) -> RuleEntry {
    let selectors: BTreeSet<Option<&'static str>> = sets.iter().map(|s| s.selector).collect();
    assert!(
        selectors.len() <= 1,
        "类名 '{}' 产出了多个不同的伴生选择器 {:?}——静态表每行只能承载一个，请升级表结构",
        class,
        selectors
    );

    RuleEntry {
        class: class.to_string(),
        selector: sets.first().and_then(|s| s.selector),
        decls: sets
            .into_iter()
            .flat_map(|s| s.decls)
            .map(|d| (d.prop, d.value.into_owned()))
            .collect(),
    }
}

pub fn resolve_entries(
    classes: &[String],
    palette: &BTreeMap<String, Vec<ColorShadeInfo>>,
) -> (RuleEntries, Vec<String>) {
    let ctx = JsonPalette(palette);
    let candidate_set: BTreeSet<&String> = classes.iter().collect();

    let mut static_entries = Vec::with_capacity(candidate_set.len());
    let mut unimplemented_entries: Vec<String> = Vec::with_capacity(candidate_set.len() / 4);

    for class in candidate_set {
        match resolve_class(class, &ctx) {
            Some(sets) => static_entries.push(flatten(class, sets)),
            None => unimplemented_entries.push(class.clone()),
        }
    }

    static_entries.sort_by(|a, b| a.class.cmp(&b.class));
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
    code.push_str("// 避免手写硬编码，与 silex_tw_core 的 resolver 保持 100% 规则对齐\n\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::ast::{Modifier, SpannedModifier, UtilityRule, UtilityValue};\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::resolver::codegen::property_id::CssPropertyId;\n");
    code.push_str("#[allow(unused_imports)]\nuse crate::css::tw::resolver::make_rule;\n");
    code.push_str("#[allow(unused_imports)]\nuse proc_macro2::Span;\n\n");
}

pub fn push_candidate_array(code: &mut String, var_name: &str, entries: &RuleEntries) {
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(code, "pub const {}: &[&str] = &[", var_name);
    for entry in entries {
        let _ = writeln!(code, "    \"{}\",", entry.class);
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
    code.push_str(
        "/// 静态表的一行：类名 + 伴生选择器（`divide-*` / `placeholder-*` 等）+ 声明列表\n",
    );
    code.push_str(
        "pub type StaticRuleRow = (&'static str, Option<&'static str>, &'static [(CssPropertyId, StaticVal)]);\n\n",
    );
    code.push_str("#[rustfmt::skip]\n");
    let _ = writeln!(code, "pub static {}: &[StaticRuleRow] = &[", var_name);
    for entry in entries {
        let selector = match entry.selector {
            Some(s) => format!("Some(\"{}\")", escape(s)),
            None => "None".to_string(),
        };
        let _ = writeln!(code, "    (\"{}\", {}, &[", entry.class, selector);
        for (prop, val) in &entry.decls {
            let static_val_str = parse_val_to_static_val(val, &mut used);
            let _ = writeln!(
                code,
                "        (CssPropertyId::{}, {}),",
                to_pascal_case(prop),
                static_val_str
            );
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

pub fn push_resolve_static_rule_fn(
    code: &mut String,
    rules_var_name: &str,
    used: &UsedStaticValKinds,
) {
    let _ = writeln!(
        code,
        r#"pub fn resolve_static_rule(
    modifiers: &[SpannedModifier],
    utility_token: &str,
    span: Span,
) -> Option<Vec<UtilityRule>> {{
    let idx = {rules_var_name}.binary_search_by_key(&utility_token, |&(k, _, _)| k).ok()?;
    let (_, selector, entries) = {rules_var_name}[idx];

    // 伴生选择器（`divide-*` 的相邻子元素、`placeholder-*` 的 `::placeholder`）
    // 以额外修饰符的形式挂上去，与模式解析路径的表达方式保持一致。
    let mut mods: smallvec::SmallVec<[SpannedModifier; 2]> = modifiers.iter().cloned().collect();
    if let Some(sel) = selector {{
        mods.push(SpannedModifier::new(Modifier::CustomSelector(sel.to_string()), span));
    }}

    let mut rules = Vec::with_capacity(entries.len());
    for &(ref prop, val) in entries {{
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
        rules.push(make_rule(mods.clone(), *prop, uval, span).ok()?);
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

fn escape(val: &str) -> String {
    val.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 把 CSS 值文本渲染成静态表里的 `StaticVal` 字面量。
///
/// 分类判定复用 `silex_tw_core::classify`——**同一个函数**也被 macro 侧的模式解析路径使用，
/// 两条路径因此不可能对同一个值给出不同的变体（报告 §3.1 的漂移根因之一）。
pub fn parse_val_to_static_val(val: &str, used: &mut UsedStaticValKinds) -> String {
    match classify(val) {
        TwValueKind::RingShadow => {
            used.ring_shadow = true;
            "StaticVal::RingShadow".to_string()
        }
        TwValueKind::Hex => {
            used.hex = true;
            format!("StaticVal::Hex(\"{}\")", escape(val))
        }
        TwValueKind::Numeric(v, unit) => {
            used.num = true;
            format!("StaticVal::Num({:?}, \"{}\")", v, unit)
        }
        TwValueKind::Literal => {
            used.literal = true;
            format!("StaticVal::Literal(\"{}\")", escape(val))
        }
        TwValueKind::Keyword => {
            used.kw = true;
            format!("StaticVal::Kw(\"{}\")", escape(val))
        }
    }
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
