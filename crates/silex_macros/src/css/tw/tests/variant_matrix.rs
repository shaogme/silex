//! 变体全表矩阵与负例测试。
//!
//! 分析报告 §6.3.3/4：
//!
//! * **矩阵**——`MODIFIER_TABLE` 全表 × 一个固定工具类做笛卡尔展开，断言生成的选择器
//!   与预期字符串**精确相等**（而非 `contains`）。§2.1 的 group 选择器缺后代组合符
//!   之所以能潜伏到线上，就是因为旧断言用 `contains(".group\\/avatar[data-size=sm]")`，
//!   有没有那个空格都能通过。
//! * **负例**——拼错的变体、尚未支持的语法**必须报错**。§2.5 修复前 `mdd:flex` 会静默
//!   降级成 `:mdd` 伪类；没有负例测试的话，这个降级随时可能被"顺手加个兜底"复活。

use super::css_probe::compile_class;
use crate::css::tw::resolver::codegen::modifiers::MODIFIER_TABLE;

/// 把 CSS 压成可逐字符比较的形式。
///
/// **保留后代组合符的空格**——那正是 §2.1 的回归点；只压掉 `>`/`~`/`+`/`,`/`{`/`}`/`:`
/// 周围的空格，这些位置的空白 LightningCSS 一定会删掉，留着只会制造假阴性。
fn normalize_css(css: &str) -> String {
    let collapsed = css.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars: Vec<char> = collapsed.chars().collect();
    let mut out = String::with_capacity(chars.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            let prev = out.chars().last();
            let next = chars.get(i + 1).copied();
            let tight = |o: Option<char>| {
                matches!(
                    o,
                    Some('>')
                        | Some('~')
                        | Some('+')
                        | Some(',')
                        | Some('{')
                        | Some('}')
                        | Some(':')
                )
            };
            if tight(prev) || tight(next) {
                continue;
            }
        }
        out.push(c);
    }
    canonicalize_selector_syntax(&out)
}

/// LightningCSS 对选择器做的**无损**改写，比较前统一抹平。
///
/// 只列真正等价的：单冒号是 CSS2 遗留写法（仅这四个伪元素有），`even`/`odd` 与
/// `2n`/`2n+1` 是同一个式子，属性值的引号在标识符合法时可省。
/// 组合符的空格**不在此列**——那正是 §2.1 的回归点，必须原样比较。
fn canonicalize_selector_syntax(css: &str) -> String {
    let mut out = css.to_string();
    for legacy in ["before", "after", "first-line", "first-letter"] {
        out = out.replace(&format!(":{legacy}"), &format!("::{legacy}"));
        out = out.replace(&format!(":::{legacy}"), &format!("::{legacy}"));
    }
    out = out
        .replace("nth-child(2n+1)", "nth-child(odd)")
        .replace("nth-child(2n)", "nth-child(even)");
    // `[dir="rtl"]` → `[dir=rtl]`
    out.replace("=\"", "=").replace("\"]", "]")
}

/// 从产物里取出生成的类名（`.slx-tw-xxxx`）
fn generated_class(css: &str) -> String {
    let start = css.find(".slx-tw-").expect("产物里应当有生成的类名");
    let rest = &css[start + 1..];
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .unwrap_or(rest.len());
    format!(".{}", &rest[..end])
}

/// 按 `ModifierMeta.css_selector` 推导出该变体应当生成的完整 CSS。
///
/// 表里不含 `&` 的条目是媒体特性（`print` / `(min-width: 1024px)` …），走 `@media`；
/// 其余是选择器变体，`&` 替换成生成的类名。
fn expected_css(css_selector: &str, class: &str) -> String {
    if css_selector.contains('&') {
        let selector = css_selector.replace('&', class);
        format!("@layer utilities{{{selector}{{display:flex}}}}")
    } else {
        format!("@layer utilities{{@media {css_selector}{{{class}{{display:flex}}}}}}")
    }
}

/// 断点变体在表里存的是媒体条件，但产物按断点排序，形式与其它媒体特性一致
#[test]
fn every_modifier_in_the_table_produces_exactly_its_selector() {
    let mut failures = Vec::new();

    for meta in MODIFIER_TABLE {
        let src = format!("{}:flex", meta.key);
        let css = match compile_class(&src) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("  {src}: 编译失败 — {e}"));
                continue;
            }
        };
        let class = generated_class(&css);
        let expected = normalize_css(&expected_css(meta.css_selector, &class));
        let actual = normalize_css(&css);
        if expected != actual {
            failures.push(format!("  {src}\n    期望 {expected}\n    实得 {actual}"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} / {} 个变体的产物与 MODIFIER_TABLE 不符：\n{}",
        failures.len(),
        MODIFIER_TABLE.len(),
        failures.join("\n")
    );
}

#[test]
fn modifier_table_covers_every_variant_kind() {
    // 矩阵测试只有在表里确实有各类变体时才有意义——表被误清空也要能发现
    assert!(
        MODIFIER_TABLE.len() >= 60,
        "MODIFIER_TABLE 只有 {} 条，疑似生成异常",
        MODIFIER_TABLE.len()
    );
    let has = |k: &str| MODIFIER_TABLE.iter().any(|m| m.key == k);
    for key in ["hover", "md", "print", "rtl", "before", "dark", "*"] {
        assert!(has(key), "MODIFIER_TABLE 缺少代表性变体 '{key}'");
    }
}

// ---------------------------------------------------------------------------
// group / peer 组合变体：表里没有，需要单独逐条钉死
// ---------------------------------------------------------------------------

#[test]
fn group_and_peer_variants_produce_exact_selectors() {
    #[rustfmt::skip]
    let cases: &[(&str, &str)] = &[
        ("group-hover:flex",                    ".group:hover CLASS"),
        ("group-focus:flex",                    ".group:focus CLASS"),
        ("group-data-[state=open]:flex",        ".group[data-state=open] CLASS"),
        ("group-aria-[expanded=true]:flex",     ".group[aria-expanded=true] CLASS"),
        ("group-has-[.x]:flex",                 ".group:has(.x) CLASS"),
        ("group-data-[size=sm]/avatar:flex",    ".group\\/avatar[data-size=sm] CLASS"),
        ("peer-hover:flex",                     ".peer:hover~CLASS"),
        ("peer-data-[state=open]:flex",         ".peer[data-state=open]~CLASS"),
        ("peer-checked/toggle:flex",            ".peer\\/toggle:checked~CLASS"),
    ];

    for &(src, expected_selector) in cases {
        let css = compile_class(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        let class = generated_class(&css);
        let expected = normalize_css(&format!(
            "@layer utilities{{{}{{display:flex}}}}",
            expected_selector.replace("CLASS", &class)
        ));
        assert_eq!(normalize_css(&css), expected, "`{src}` 的选择器与预期不符");
    }
}

#[test]
fn stacked_modifiers_nest_in_priority_order() {
    // 断点在外、伪类在内——顺序错了会让 `md:hover:` 与 `hover:md:` 产出不同结果
    let css = compile_class("md:hover:flex").unwrap();
    let class = generated_class(&css);
    assert_eq!(
        normalize_css(&css),
        normalize_css(&format!(
            "@layer utilities{{@media (min-width:768px){{{class}:hover{{display:flex}}}}}}"
        ))
    );
}

// ---------------------------------------------------------------------------
// 负例：这些**必须**报错
// ---------------------------------------------------------------------------

/// `(源码, 错误信息里必须出现的片段)`
#[rustfmt::skip]
const MUST_FAIL: &[(&str, &str)] = &[
    // §2.5 拼错的变体前缀不得静默降级为伪类
    ("mdd:flex",                   "Unknown variant prefix"),
    ("hoveer:flex",                "Unknown variant prefix"),
    ("focuss:text-red-500",        "Unknown variant prefix"),
    // §13.1 函数式变体已支持，但参数写坏了仍必须报错——支持一个家族不等于
    // 放弃校验它的参数，否则又回到"静默产出永不匹配的选择器"
    ("supports-grid:flex",         "brackets"),
    ("supports-[]:flex",           "empty feature query"),
    ("max-notabreakpoint:flex",    "unknown breakpoint"),
    ("min-[]:flex",                "empty width"),
    ("nth-abc:flex",               "invalid"),
    ("not-before:flex",            "cannot be negated"),
    ("not-notavariant:flex",       "unknown variant"),
    ("in-print:flex",              "needs a variant that"),
    // §7 尚未支持的工具类语法，必须报错而不是产出错误 CSS
    ("p-44x",                      "p-4"),
    ("bg-notacolor-500",           ""),
];

#[test]
fn unsupported_syntax_is_rejected_not_silently_downgraded() {
    let mut leaked = Vec::new();
    for &(src, needle) in MUST_FAIL {
        match compile_class(src) {
            Ok(css) => leaked.push(format!("  `{src}` 本应报错，却编译成了: {}", css.trim())),
            Err(msg) => {
                if !needle.is_empty() && !msg.contains(needle) {
                    leaked.push(format!(
                        "  `{src}` 的错误信息缺少 `{needle}`，实际是: {msg}"
                    ));
                }
            }
        }
    }
    assert!(leaked.is_empty(), "负例未被拦截：\n{}", leaked.join("\n"));
}

#[test]
fn rejected_variants_never_become_pseudo_classes() {
    // 回归点：兜底分支曾把任意未知前缀原样拼成 `:xxx`，产出永不匹配的选择器。
    // 这里从产物侧再钉一次——即便将来有人加回兜底，也会被抓住。
    for src in ["mdd:flex", "maxx-md:flex", "nthh-3:flex"] {
        if let Ok(css) = compile_class(src) {
            panic!("`{src}` 不应编译成功，产物: {css}");
        }
    }
}

// ---------------------------------------------------------------------------
// 函数式变体（第四阶段第 15 项）：期望值取自真实 `tailwindcss@4.3.3` 的
// `designSystem.candidatesToCss()`，逐字符相等比较
// ---------------------------------------------------------------------------

/// `(源码, 期望产物)`。`CLASS` 占位生成的类名；`@media`/`@supports` 的条件按
/// LightningCSS 对当前 targets 的降级结果书写（范围语法 → `min-width` / `not`）。
#[rustfmt::skip]
const FUNCTIONAL_VARIANTS: &[(&str, &str)] = &[
    // supports-[…]：带值的探测按原样，只给属性名的用 Tailwind 自己的哑值写法
    ("supports-[display:grid]:flex",     "@supports (display:grid){CLASS{display:flex}}"),
    ("supports-[display:_grid]:flex",    "@supports (display: grid){CLASS{display:flex}}"),
    ("supports-[backdrop-filter]:flex",  "@supports (backdrop-filter:var(--tw)){CLASS{display:flex}}"),
    ("not-supports-[display:grid]:flex", "@supports not (display:grid){CLASS{display:flex}}"),
    // min-* / max-*：源码写的是范围语法，LightningCSS 按 targets 降级
    ("min-[600px]:flex",                 "@media (min-width:600px){CLASS{display:flex}}"),
    ("min-md:flex",                      "@media (min-width:768px){CLASS{display:flex}}"),
    ("max-md:flex",                      "@media not (min-width:768px){CLASS{display:flex}}"),
    ("max-[600px]:flex",                 "@media not (min-width:600px){CLASS{display:flex}}"),
    // not-*：选择器类取 :not()，媒体类取 @media not
    ("not-hover:flex",                   "CLASS:not(:hover){display:flex}"),
    ("not-first:flex",                   "CLASS:not(:first-child){display:flex}"),
    ("not-open:flex",                    "CLASS:not(:is([open],:popover-open,:open)){display:flex}"),
    ("not-data-[state=open]:flex",       "CLASS:not([data-state=open]){display:flex}"),
    ("not-md:flex",                      "@media not (min-width:768px){CLASS{display:flex}}"),
    ("not-print:flex",                   "@media not print{CLASS{display:flex}}"),
    // in-*：祖先无需 marker 类
    ("in-focus:flex",                    ":where(:focus) CLASS{display:flex}"),
    ("in-[.card]:flex",                  ":where(.card) CLASS{display:flex}"),
    ("in-data-[state=open]:flex",        ":where([data-state=open]) CLASS{display:flex}"),
    // nth-*
    ("nth-3:flex",                       "CLASS:nth-child(3){display:flex}"),
    ("nth-last-3:flex",                  "CLASS:nth-last-child(3){display:flex}"),
    ("nth-of-type-3:flex",               "CLASS:nth-of-type(3){display:flex}"),
    ("nth-last-of-type-3:flex",          "CLASS:nth-last-of-type(3){display:flex}"),
    ("nth-[2n+1]:flex",                  "CLASS:nth-child(odd){display:flex}"),
    // @starting-style
    ("starting:opacity-0",               "@starting-style{CLASS{opacity:0}}"),
    // has-*：`has-not-` 此前会退化成 `:has(has-not-[.x])`
    ("has-checked:flex",                 "CLASS:has(:checked){display:flex}"),
    ("has-not-[.x]:flex",                "CLASS:has(:not(.x)){display:flex}"),
    ("has-not-checked:flex",             "CLASS:has(:not(:checked)){display:flex}"),
];

#[test]
fn functional_variants_produce_exactly_tailwinds_selectors() {
    let mut failures = Vec::new();
    for &(src, expected_body) in FUNCTIONAL_VARIANTS {
        let css = match compile_class(src) {
            Ok(c) => c,
            Err(e) => {
                failures.push(format!("  {src}: 编译失败 — {e}"));
                continue;
            }
        };
        let class = generated_class(&css);
        let expected = normalize_css(&format!(
            "@layer utilities{{{}}}",
            expected_body.replace("CLASS", &class)
        ));
        let actual = normalize_css(&css);
        if expected != actual {
            failures.push(format!("  {src}\n    期望 {expected}\n    实得 {actual}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} 个函数式变体的产物与预期不符：\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn overlapping_max_width_variants_sort_from_wide_to_narrow() {
    // `max-lg` 与 `max-md` 在 700px 处同时命中，窄的必须写在后面才能覆盖宽的。
    // 关键是这个顺序**不依赖源码书写顺序**——这里故意把 max-md 写在前面。
    let css = compile_class("max-md:p-4 max-lg:p-2").expect("应当编译成功");
    let lg = css.find("1024px").expect("产物里应有 max-lg 的条件");
    let md = css.find("768px").expect("产物里应有 max-md 的条件");
    assert!(
        lg < md,
        "max-lg 必须排在 max-md 之前（窄的覆盖宽的），实得:\n{css}"
    );
}

#[test]
fn arbitrary_pseudo_class_remains_the_explicit_escape_hatch() {
    // 拒绝未知前缀的同时必须保留显式透传的出口，否则用户无路可走
    let css = compile_class("[&:my-pseudo]:flex").expect("显式任意伪类应当可用");
    let class = generated_class(&css);
    assert_eq!(
        normalize_css(&css),
        normalize_css(&format!(
            "@layer utilities{{{class}:my-pseudo{{display:flex}}}}"
        ))
    );
}
