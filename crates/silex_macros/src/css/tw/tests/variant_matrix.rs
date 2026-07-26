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
    // §2.5 Tailwind 有、本实现尚未支持的函数式变体，要给出"该家族未支持"的明确提示
    ("max-md:flex",                "not supported yet"),
    ("min-[600px]:flex",           "not supported yet"),
    ("supports-[display:grid]:grid", "not supported yet"),
    ("not-hover:flex",             "not supported yet"),
    ("in-focus:flex",              "not supported yet"),
    ("nth-3:flex",                 "not supported yet"),
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
    for src in ["mdd:flex", "max-md:flex", "nth-3:flex"] {
        if let Ok(css) = compile_class(src) {
            panic!("`{src}` 不应编译成功，产物: {css}");
        }
    }
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
