//! 端到端测试：`table_examples.rs` 里的每条期望规则都必须**精确**出现在编译产物中。
//!
//! 分析报告 §6.1 指出旧版断言几乎恒真——`border-*` 系列只要 CSS 里出现 `"border"` 就算通过，
//! `StaticVal::Num` 分支干脆不校验数值，`rgba`/`shadow` 字样直接放行。于是
//! `border-s-[3px]` 产出 `syntax:3px` 这样的垃圾也能绿灯。
//!
//! 现在改为：把产物解析成声明表，逐条断言"期望的属性存在，且值等价"。
//! 值的比较复用 [`super::css_probe`] 的规范化器（只抹平 LightningCSS 的无损改写）。
//!
//! 注意本文件的夹具与被测数据同源（都由 codegen 从同一份 JSON 生成，见报告 §6.2），
//! 它守护的是"解析 → 生成 → 编译"这条链路不退化；**语义**是否符合 Tailwind
//! 由 [`super::differential`] 用真实 tailwindcss 的产物来守护。

use super::css_probe::{
    class_declarations, compile_class, decls_to_map, normalize_value, values_equivalent,
};
use crate::css::tw::resolver::codegen::table_examples::{
    StaticVal, TEST_CASE_CANDIDATE_UTILITIES, TEST_CASE_RULES,
};

/// silex 与 LightningCSS 会把等价的属性写法互换，这些是**无损**的合并/展开。
///
/// 与 §6.1 批评的旧断言的区别：旧版是"出现 `border` 子串即通过"，
/// 这里是"该属性缺失时，允许由列出的等价属性以**相同的值**代为满足"——
/// 值仍然逐条校验，写错值照样失败。
#[rustfmt::skip]
const SHORTHAND_FALLBACKS: &[(&str, &[&str])] = &[
    ("padding-top",    &["padding-block", "padding"]),
    ("padding-bottom", &["padding-block", "padding"]),
    ("padding-left",   &["padding-inline", "padding"]),
    ("padding-right",  &["padding-inline", "padding"]),
    ("margin-top",     &["margin-block", "margin"]),
    ("margin-bottom",  &["margin-block", "margin"]),
    ("margin-left",    &["margin-inline", "margin"]),
    ("margin-right",   &["margin-inline", "margin"]),
    ("top",    &["inset-block", "inset"]),
    ("bottom", &["inset-block", "inset"]),
    ("left",   &["inset-inline", "inset"]),
    ("right",  &["inset-inline", "inset"]),
    ("border-top-width",    &["border-block-width", "border-width"]),
    ("border-bottom-width", &["border-block-width", "border-width"]),
    ("border-left-width",   &["border-inline-width", "border-width"]),
    ("border-right-width",  &["border-inline-width", "border-width"]),
    ("border-block-start-width",  &["border-block-width", "border-width"]),
    ("border-block-end-width",    &["border-block-width", "border-width"]),
    ("border-inline-start-width", &["border-inline-width", "border-width"]),
    ("border-inline-end-width",   &["border-inline-width", "border-width"]),
    ("scroll-margin-top",    &["scroll-margin-block", "scroll-margin"]),
    ("scroll-margin-bottom", &["scroll-margin-block", "scroll-margin"]),
    ("scroll-margin-left",   &["scroll-margin-inline", "scroll-margin"]),
    ("scroll-margin-right",  &["scroll-margin-inline", "scroll-margin"]),
    ("scroll-padding-top",    &["scroll-padding-block", "scroll-padding"]),
    ("scroll-padding-bottom", &["scroll-padding-block", "scroll-padding"]),
    ("scroll-padding-left",   &["scroll-padding-inline", "scroll-padding"]),
    ("scroll-padding-right",  &["scroll-padding-inline", "scroll-padding"]),
    ("row-gap",    &["gap"]),
    ("column-gap", &["gap"]),
    ("width",  &["inline-size"]),
    ("height", &["block-size"]),
];

/// 把夹具里的期望值渲染成待比较的字符串。
///
/// `RingShadow` 是唯一无法用字面量表达的情况——它是一长串 `var(--tw-ring-*)` 组合，
/// 只断言 ring 变量体系被启用。
fn expected_value(val: &StaticVal) -> Option<String> {
    match val {
        StaticVal::Kw(k) => Some(normalize_value(k)),
        StaticVal::Literal(l) => Some(normalize_value(l)),
        StaticVal::Num(v, unit) => Some(normalize_value(&format!("{v}{unit}"))),
        StaticVal::RingShadow => None,
    }
}

#[test]
fn table_examples_compile_to_exactly_the_expected_declarations() {
    let mut failures = Vec::new();

    // 伴生选择器（`placeholder-*` 的 `::placeholder` 等）在这里不参与断言——
    // 声明落在哪个选择器上由 `variant_matrix` 的选择器精确断言守护。
    for &(class_name, _selector, expected_rules) in TEST_CASE_RULES {
        let decls = match class_declarations(class_name) {
            Ok(d) => d,
            Err(e) => {
                failures.push(format!("  {class_name}: 编译失败 — {e}"));
                continue;
            }
        };
        let actual = decls_to_map(decls.iter().map(|(p, v)| (p.as_str(), v.as_str())));

        for (prop_id, val) in expected_rules {
            let prop = prop_id.as_str();

            // ring 的 box-shadow 是变量组合，只断言体系被启用
            let Some(expected) = expected_value(val) else {
                if !decls
                    .iter()
                    .any(|(p, v)| p.starts_with("--tw-ring") || v.contains("--tw-ring"))
                {
                    failures.push(format!(
                        "  {class_name}: 期望启用 ring 变量体系，产物里没有任何 --tw-ring-*\n    产物: {decls:?}"
                    ));
                }
                continue;
            };

            let satisfied_by = |candidate: &str| {
                actual.get(candidate).is_some_and(|vals| {
                    vals.iter()
                        .any(|a| values_equivalent(candidate, &expected, a))
                })
            };

            if satisfied_by(prop) {
                continue;
            }
            let fallback = SHORTHAND_FALLBACKS
                .iter()
                .find(|(p, _)| *p == prop)
                .is_some_and(|(_, alts)| alts.iter().any(|alt| satisfied_by(alt)));
            if fallback {
                continue;
            }

            failures.push(match actual.get(prop) {
                Some(vals) => format!("  {class_name}: {prop} 期望 `{expected}`，实得 {vals:?}"),
                None => format!(
                    "  {class_name}: 缺少属性 {prop}（期望 `{expected}`）\n    产物: {decls:?}"
                ),
            });
        }
    }

    assert!(
        failures.is_empty(),
        "{} 条期望规则未被产物满足：\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn table_examples_never_emit_at_rule_descriptors() {
    // 回归点（报告 §2.3）：探针曾把 `@property` 块里的描述符当成 target_props，
    // 于是 `border-s-[3px]` 产出 `syntax:3px`。旧断言对此完全无感。
    let mut offenders = Vec::new();
    for &class_name in TEST_CASE_CANDIDATE_UTILITIES {
        let Ok(decls) = class_declarations(class_name) else {
            continue;
        };
        for (prop, value) in &decls {
            if matches!(prop.as_str(), "syntax" | "inherits" | "initial-value") {
                offenders.push(format!("  {class_name}: {prop}:{value}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "产物中混入了 at-rule 描述符声明：\n{}",
        offenders.join("\n")
    );
}

#[test]
fn batch_compilation_is_deterministic() {
    // 报告 §2.10：`deduplicate_utility_rules` 曾遍历 `HashMap`，跨修饰符组调用会产出
    // **不确定的 CSS 与类名哈希**。批量路径正是触发去重的地方，这里锁定它的确定性。
    //
    // 批量产物无法逐条断言——40 个类名合并后 LightningCSS 会合成简写
    // （`border-top-width`+`border-bottom-width` → `border-width: 4px 4px 8px`），
    // 与单独编译的声明不再一一对应。精确的值断言由上面的逐类名测试负责。
    for chunk in TEST_CASE_CANDIDATE_UTILITIES.chunks(40) {
        let batch = chunk.join(" ");
        let first = compile_class(&batch).unwrap_or_else(|e| panic!("批量编译失败: {batch}\n{e}"));
        let second = compile_class(&batch).expect("第二次编译同样应当成功");
        assert_eq!(
            first, second,
            "同一批词条两次编译产出不同的 CSS（批次起始于 {}）",
            chunk[0]
        );
        assert!(
            !super::css_probe::extract_declarations(&first).is_empty(),
            "批量编译产出了不含任何声明的 CSS（批次起始于 {}）",
            chunk[0]
        );
    }
}

#[test]
fn tw_merge_keeps_the_last_writer_of_a_property() {
    // tw-merge 的核心契约：同一属性后写覆盖先写，且覆盖是在**编译期**完成的
    // （产物里只留一条），而不是靠运行时层叠。用小而明确的冲突组断言精确结果。
    #[rustfmt::skip]
    let cases: &[(&str, &str, &str)] = &[
        ("p-4 p-8",                 "padding",     "2rem"),
        ("text-red-500 text-blue-500", "color",    "oklch(62.3% 0.214 259.815)"),
        ("inset-x-0 left-4",        "left",        "1rem"),
        ("blur-sm blur-lg",         "filter",      "blur(16px)"),
        // `none` 是整个属性的关键字取值，会清空前面累积的函数
        ("blur-sm blur-none",       "filter",      "none"),
        // 不同函数互不覆盖，仍然拼接（LightningCSS 会压掉函数之间可省的空格）
        ("blur-sm brightness-50",   "filter",      "blur(4px)brightness(.5)"),
    ];

    for &(src, prop, expected) in cases {
        let decls = class_declarations(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        let values: Vec<&str> = decls
            .iter()
            .filter(|(p, _)| p == prop)
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(
            values.len(),
            1,
            "`{src}` 应当只留下一条 {prop} 声明，实得 {values:?}"
        );
        let expected = normalize_value(expected);
        let actual = normalize_value(values[0]);
        assert!(
            values_equivalent(prop, &expected, &actual),
            "`{src}` 的 {prop} 期望 `{expected}`，实得 `{actual}`"
        );
    }
}
