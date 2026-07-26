//! Tailwind v4 函数式变体：`not-*` / `in-*` / `nth-*` / `min-*` / `max-*` / `supports-*` / `starting`
//!
//! 这些变体的共同点是**前缀带参数**，无法像 `hover` / `md` 那样在 `MODIFIER_TABLE` 里枚举，
//! 因此不进生成表，由本模块手写解析。产出一律落到既有的 `Modifier` 变体上
//! （选择器类走 `SelectorVariant`，条件块类走 `AtRuleCondition`），
//! `codegen.rs` 无需为每个家族各写一条分支。
//!
//! `not-*` 的实现方式是**递归解析内层变体再取反**，而不是另建一张 "可取反前缀" 表：
//! 后者一定会与 `MODIFIER_TABLE` 漂移（报告 §3.1 讲的正是同一语义两份实现的下场），
//! 前者则天然覆盖 `not-hover` / `not-md` / `not-print` / `not-open` / `not-supports-[…]`
//! 的全部组合，将来往表里加变体也自动获得 `not-` 形式。

use proc_macro2::Span;
use syn::{Error, Result};

use crate::css::{
    config::get_config,
    tw::{ast::Modifier, resolver::codegen::modifiers::lookup_modifier_meta},
};

/// `max-*` / `not-<断点>` 的排序基准。
///
/// `min-width` 类变体按宽度**升序**排列（`1000 + px`，与 `MODIFIER_TABLE` 的断点权重同源），
/// `max-width` 类必须**降序**——`max-lg` 与 `max-md` 在 700px 处同时命中，
/// 窄的那条要写在后面才能覆盖宽的那条。整体排在全部 min 之后（min 上限约 2536）。
const MAX_WIDTH_PRIORITY_BASE: u32 = 20_000;

/// 解析函数式变体前缀。
///
/// 返回 `Ok(None)` 表示"不是函数式变体"，调用方继续走后面的未知前缀报错逻辑。
pub(crate) fn parse_functional_modifier(prefix: &str, span: Span) -> Result<Option<Modifier>> {
    if prefix == "starting" {
        return Ok(Some(Modifier::AtRuleCondition {
            at_rule: "starting-style",
            condition: String::new(),
            priority: 68,
        }));
    }

    if let Some(rest) = prefix.strip_prefix("supports-") {
        return parse_supports(rest, span).map(Some);
    }
    if let Some(rest) = prefix.strip_prefix("min-") {
        return parse_width_variant(rest, false, span).map(Some);
    }
    if let Some(rest) = prefix.strip_prefix("max-") {
        return parse_width_variant(rest, true, span).map(Some);
    }
    if let Some(rest) = prefix.strip_prefix("nth-") {
        return parse_nth(rest, span).map(Some);
    }
    if let Some(rest) = prefix.strip_prefix("in-") {
        return parse_in(rest, span).map(Some);
    }
    if let Some(rest) = prefix.strip_prefix("not-") {
        return parse_not(rest, span).map(Some);
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// supports-[…]
// ---------------------------------------------------------------------------

/// `supports-[display:grid]` → `@supports (display: grid)`
///
/// 只给出属性名时（`supports-[backdrop-filter]`）Tailwind 探测的是"该属性是否被识别"，
/// 用一个恒不成立的哑值 `var(--tw)` 表达——这是 Tailwind 自己的写法，照搬以保持一致。
fn parse_supports(rest: &str, span: Span) -> Result<Modifier> {
    let inner = bracketed_inner(rest).ok_or_else(|| {
        Error::new(
            span,
            format!(
                "Variant 'supports-{}:' is malformed. Write the feature query in brackets, \
                 e.g. `supports-[display:grid]:` or `supports-[backdrop-filter]:`.",
                rest
            ),
        )
    })?;

    let inner = inner.replace('_', " ");
    let inner = inner.trim();
    let inner = inner
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(inner)
        .trim();
    if inner.is_empty() {
        return Err(Error::new(
            span,
            "Variant 'supports-[]:' has an empty feature query.",
        ));
    }

    let condition = if inner.contains(':') {
        format!("({})", inner)
    } else {
        format!("({}: var(--tw))", inner)
    };

    Ok(Modifier::AtRuleCondition {
        at_rule: "supports",
        condition,
        priority: 66,
    })
}

// ---------------------------------------------------------------------------
// min-* / max-*
// ---------------------------------------------------------------------------

/// `min-[600px]` / `min-md` / `max-[600px]` / `max-md`
///
/// 用范围语法（`(width >= 600px)` / `(width < 600px)`）而不是 `min-width` / `max-width`：
/// `max-md` 的边界必须是**开区间**，写成 `(max-width: 768px)` 会与 `md:` 在 768px 处重叠。
/// 老浏览器的降级由 LightningCSS 按 targets 处理。
fn parse_width_variant(rest: &str, is_max: bool, span: Span) -> Result<Modifier> {
    let (width, px) = match bracketed_inner(rest) {
        Some(inner) => {
            let inner = inner.trim();
            if inner.is_empty() {
                return Err(Error::new(
                    span,
                    format!(
                        "Variant '{}-[]:' has an empty width.",
                        if is_max { "max" } else { "min" }
                    ),
                ));
            }
            (inner.to_string(), parse_css_length_px(inner))
        }
        None => {
            let Some(width) = lookup_breakpoint_width(rest) else {
                return Err(Error::new(
                    span,
                    format!(
                        "Variant '{}-{}:' refers to an unknown breakpoint '{}'. \
                         Use a configured breakpoint name or an explicit width such as `{}-[600px]:`.",
                        if is_max { "max" } else { "min" },
                        rest,
                        rest,
                        if is_max { "max" } else { "min" }
                    ),
                ));
            };
            let px = parse_css_length_px(&width);
            (width, px)
        }
    };

    let px = px.unwrap_or(640.0).round() as u32;
    let (condition, priority) = if is_max {
        (
            format!("(width < {})", width),
            MAX_WIDTH_PRIORITY_BASE.saturating_sub(px),
        )
    } else {
        (format!("(width >= {})", width), 1000 + px)
    };

    Ok(Modifier::AtRuleCondition {
        at_rule: "media",
        condition,
        priority,
    })
}

/// 取断点名对应的宽度：先查生成表里的 `(min-width: 768px)`，再查 `silex.toml`
fn lookup_breakpoint_width(name: &str) -> Option<String> {
    if let Some(meta) = lookup_modifier_meta(name)
        && let Some(width) = meta
            .css_selector
            .strip_prefix("(min-width:")
            .and_then(|s| s.strip_suffix(')'))
    {
        return Some(width.trim().to_string());
    }
    get_config().and_then(|cfg| cfg.theme.breakpoints.get(name).cloned())
}

/// 取 CSS 长度的像素值，仅用于排序权重（`rem` 按 16px 折算）
fn parse_css_length_px(s: &str) -> Option<f64> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("px") {
        return v.trim().parse::<f64>().ok();
    }
    if let Some(v) = s.strip_suffix("rem") {
        return v.trim().parse::<f64>().ok().map(|v| v * 16.0);
    }
    if let Some(v) = s.strip_suffix("em") {
        return v.trim().parse::<f64>().ok().map(|v| v * 16.0);
    }
    s.parse::<f64>().ok()
}

// ---------------------------------------------------------------------------
// nth-*
// ---------------------------------------------------------------------------

/// `nth-3` / `nth-[2n+1]` / `nth-last-3` / `nth-of-type-3` / `nth-last-of-type-3`
fn parse_nth(rest: &str, span: Span) -> Result<Modifier> {
    // 长前缀优先，否则 `nth-last-of-type-3` 会被 `last-` 先吃掉
    const KINDS: &[(&str, &str)] = &[
        ("last-of-type-", "nth-last-of-type"),
        ("of-type-", "nth-of-type"),
        ("last-", "nth-last-child"),
    ];

    let (arg, pseudo) = KINDS
        .iter()
        .find_map(|&(p, pseudo)| rest.strip_prefix(p).map(|arg| (arg, pseudo)))
        .unwrap_or((rest, "nth-child"));

    let arg = match bracketed_inner(arg) {
        Some(inner) => inner.replace('_', " ").trim().to_string(),
        None => arg.to_string(),
    };

    if !is_valid_nth_arg(&arg) {
        return Err(Error::new(
            span,
            format!(
                "Variant 'nth-{}:' has an invalid `{}()` argument '{}'. \
                 Expected a positive integer or an An+B expression such as `nth-[2n+1]:`.",
                rest, pseudo, arg
            ),
        ));
    }

    Ok(Modifier::SelectorVariant(format!("&:{}({})", pseudo, arg)))
}

/// 校验 `An+B` 形式：`3` / `2n` / `2n+1` / `-n+3` / `odd` / `even`
fn is_valid_nth_arg(arg: &str) -> bool {
    if arg.is_empty() {
        return false;
    }
    if arg.eq_ignore_ascii_case("odd") || arg.eq_ignore_ascii_case("even") {
        return true;
    }
    arg.chars()
        .all(|c| c.is_ascii_digit() || matches!(c, 'n' | 'N' | '+' | '-' | ' '))
        && arg
            .chars()
            .any(|c| c.is_ascii_digit() || c == 'n' || c == 'N')
}

// ---------------------------------------------------------------------------
// in-*
// ---------------------------------------------------------------------------

/// `in-focus` → `:where(:focus) &`（任意祖先满足条件）
///
/// 与 `group-*` 的区别：`group-*` 要求祖先带 `.group` 标记类，`in-*` 不要求标记。
fn parse_in(rest: &str, span: Span) -> Result<Modifier> {
    let inner = super::parser::parse_single_modifier(rest, span).map_err(|_| {
        Error::new(
            span,
            format!(
                "Variant 'in-{}:' refers to an unknown variant '{}'. \
                 `in-*` takes a selector variant, e.g. `in-focus:` or `in-[.card]:`.",
                rest, rest
            ),
        )
    })?;

    let selector = self_selector_of(&inner).ok_or_else(|| {
        Error::new(
            span,
            format!(
                "Variant 'in-{}:' cannot be built from '{}': `in-*` needs a variant that \
                 selects an element (a pseudo-class, attribute or arbitrary selector), \
                 not a media query or a group/peer variant.",
                rest, rest
            ),
        )
    })?;

    Ok(Modifier::SelectorVariant(format!(":where({}) &", selector)))
}

// ---------------------------------------------------------------------------
// not-*
// ---------------------------------------------------------------------------

/// `not-hover` → `&:not(:hover)`；`not-md` → `@media not (min-width: 768px)`
fn parse_not(rest: &str, span: Span) -> Result<Modifier> {
    let inner = super::parser::parse_single_modifier(rest, span).map_err(|_| {
        Error::new(
            span,
            format!(
                "Variant 'not-{}:' refers to an unknown variant '{}'.",
                rest, rest
            ),
        )
    })?;
    negate_modifier(&inner, rest, span)
}

/// 取变体作用在**元素自身**上的选择器（去掉前导 `&`），媒体/容器类返回 `None`
fn self_selector_of(m: &Modifier) -> Option<String> {
    let sel = match m {
        Modifier::PseudoClass(k) => lookup_modifier_meta(k)
            .map(|meta| meta.css_selector.to_string())
            .unwrap_or_else(|| format!("&:{k}")),
        Modifier::SelectorVariant(s) => s.clone(),
        Modifier::CustomSelector(s) => s.clone(),
        Modifier::DataAttribute { key, value } => match value {
            Some(v) => format!("&[data-{}=\"{}\"]", key, v),
            None => format!("&[data-{}]", key),
        },
        Modifier::AriaAttribute { key, value } => match value {
            Some(v) => format!("&[aria-{}=\"{}\"]", key, v),
            None => format!("&[aria-{}]", key),
        },
        Modifier::Has(target) => format!("&:has({})", crate::css::tw::codegen::has_target(target)),
        _ => return None,
    };
    let stripped = sel.strip_prefix('&').unwrap_or(&sel).trim().to_string();
    if stripped.is_empty() || stripped.contains('&') {
        // 形如 `.dark &` 的复合选择器不是"作用在自身"，交给各自的分支处理
        return None;
    }
    Some(stripped)
}

/// 对一个已解析的变体取反
fn negate_modifier(m: &Modifier, raw: &str, span: Span) -> Result<Modifier> {
    match m {
        Modifier::MediaBreakpoint(bp) => {
            let width = lookup_breakpoint_width(bp).unwrap_or_else(|| "640px".to_string());
            let px = parse_css_length_px(&width).unwrap_or(640.0).round() as u32;
            Ok(Modifier::AtRuleCondition {
                at_rule: "media",
                condition: format!("not (min-width: {})", width),
                priority: MAX_WIDTH_PRIORITY_BASE.saturating_sub(px),
            })
        }
        Modifier::MediaQuery(q) => Ok(Modifier::AtRuleCondition {
            at_rule: "media",
            condition: format!("not {}", q),
            priority: 65,
        }),
        Modifier::AtRuleCondition {
            at_rule,
            condition,
            priority,
        } => Ok(Modifier::AtRuleCondition {
            at_rule,
            condition: match condition.strip_prefix("not ") {
                // 双重否定直接消掉，别产出 `not not (…)`（CSS 不接受）
                Some(inner) => inner.to_string(),
                None if condition.is_empty() => {
                    return Err(Error::new(
                        span,
                        format!("Variant 'not-{}:' cannot be negated.", raw),
                    ));
                }
                None => format!("not {}", condition),
            },
            priority: *priority,
        }),
        Modifier::Dark => {
            let dark_mode = get_config()
                .and_then(|cfg| cfg.theme.dark_mode.as_deref())
                .unwrap_or("class");
            if dark_mode == "media" {
                Ok(Modifier::AtRuleCondition {
                    at_rule: "media",
                    condition: "not (prefers-color-scheme: dark)".to_string(),
                    priority: 60,
                })
            } else {
                // `dark` 是 `.dark &, &.dark`；取反要同时排除祖先带 .dark 与自身带 .dark
                Ok(Modifier::SelectorVariant(
                    "&:not(.dark *):not(.dark)".to_string(),
                ))
            }
        }
        _ => {
            let selector = self_selector_of(m).ok_or_else(|| {
                Error::new(
                    span,
                    format!(
                        "Variant 'not-{}:' cannot be negated: '{}' is not a variant that \
                         selects an element or a conditional at-rule.",
                        raw, raw
                    ),
                )
            })?;
            Ok(Modifier::SelectorVariant(format!("&:not({})", selector)))
        }
    }
}

// ---------------------------------------------------------------------------
// 公共小工具
// ---------------------------------------------------------------------------

/// 取 `[...]` 的内容
fn bracketed_inner(s: &str) -> Option<&str> {
    s.strip_prefix('[')?.strip_suffix(']')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(prefix: &str) -> Modifier {
        parse_functional_modifier(prefix, Span::call_site())
            .unwrap_or_else(|e| panic!("{prefix}: {e}"))
            .unwrap_or_else(|| panic!("{prefix} 未被识别为函数式变体"))
    }

    fn err(prefix: &str) -> String {
        parse_functional_modifier(prefix, Span::call_site())
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| panic!("{prefix} 本应报错"))
    }

    #[test]
    fn supports_variants_wrap_the_feature_query() {
        assert_eq!(
            parse("supports-[display:grid]"),
            Modifier::AtRuleCondition {
                at_rule: "supports",
                condition: "(display:grid)".to_string(),
                priority: 66,
            }
        );
        // 只给属性名时探测"属性是否被识别"，用 Tailwind 自己的哑值写法
        assert_eq!(
            parse("supports-[backdrop-filter]"),
            Modifier::AtRuleCondition {
                at_rule: "supports",
                condition: "(backdrop-filter: var(--tw))".to_string(),
                priority: 66,
            }
        );
        // 下划线是 Tailwind 任意值里的空格
        assert_eq!(
            parse("supports-[display:_grid]"),
            Modifier::AtRuleCondition {
                at_rule: "supports",
                condition: "(display: grid)".to_string(),
                priority: 66,
            }
        );
        assert!(err("supports-grid").contains("Write the feature query in brackets"));
    }

    #[test]
    fn width_variants_use_open_intervals_and_sortable_priorities() {
        let min_600 = parse("min-[600px]");
        assert_eq!(
            min_600,
            Modifier::AtRuleCondition {
                at_rule: "media",
                condition: "(width >= 600px)".to_string(),
                priority: 1600,
            }
        );
        // `max-md` 的边界必须是开区间，否则与 `md:` 在 768px 处同时命中
        assert_eq!(
            parse("max-md"),
            Modifier::AtRuleCondition {
                at_rule: "media",
                condition: "(width < 768px)".to_string(),
                priority: MAX_WIDTH_PRIORITY_BASE - 768,
            }
        );
        // max 系按宽度降序：窄的排在后面才能覆盖宽的
        let (
            Modifier::AtRuleCondition { priority: p_md, .. },
            Modifier::AtRuleCondition { priority: p_lg, .. },
        ) = (parse("max-md"), parse("max-lg"))
        else {
            panic!("期望 AtRuleCondition");
        };
        assert!(p_lg < p_md, "max-lg 必须排在 max-md 之前");
        // 且全部 max 排在全部 min 之后
        let Modifier::AtRuleCondition {
            priority: p_min, ..
        } = parse("min-[1536px]")
        else {
            panic!("期望 AtRuleCondition");
        };
        assert!(p_min < p_lg);

        assert!(err("max-notabreakpoint").contains("unknown breakpoint"));
    }

    #[test]
    fn nth_variants_map_to_the_matching_pseudo_class() {
        let cases = [
            ("nth-3", "&:nth-child(3)"),
            ("nth-[2n+1]", "&:nth-child(2n+1)"),
            ("nth-[odd]", "&:nth-child(odd)"),
            ("nth-last-3", "&:nth-last-child(3)"),
            ("nth-of-type-3", "&:nth-of-type(3)"),
            ("nth-last-of-type-3", "&:nth-last-of-type(3)"),
        ];
        for (prefix, expected) in cases {
            assert_eq!(parse(prefix), Modifier::SelectorVariant(expected.into()));
        }
        assert!(err("nth-abc").contains("invalid"));
    }

    #[test]
    fn in_variants_select_an_unmarked_ancestor() {
        assert_eq!(
            parse("in-focus"),
            Modifier::SelectorVariant(":where(:focus) &".into())
        );
        assert_eq!(
            parse("in-[.card]"),
            Modifier::SelectorVariant(":where(.card) &".into())
        );
        // 媒体查询没有"祖先"可言
        assert!(err("in-print").contains("needs a variant that"));
    }

    #[test]
    fn not_variants_negate_the_inner_variant() {
        // 伪类 → :not(…)，且用的是表里的真实选择器（`first` 是 `:first-child`）
        assert_eq!(
            parse("not-hover"),
            Modifier::SelectorVariant("&:not(:hover)".into())
        );
        assert_eq!(
            parse("not-first"),
            Modifier::SelectorVariant("&:not(:first-child)".into())
        );
        assert_eq!(
            parse("not-open"),
            Modifier::SelectorVariant("&:not(:is([open], :popover-open, :open))".into())
        );
        assert_eq!(
            parse("not-data-[state=open]"),
            Modifier::SelectorVariant("&:not([data-state=\"open\"])".into())
        );
        // 媒体类 → @media not …
        assert_eq!(
            parse("not-print"),
            Modifier::AtRuleCondition {
                at_rule: "media",
                condition: "not print".to_string(),
                priority: 65,
            }
        );
        assert_eq!(
            parse("not-md"),
            Modifier::AtRuleCondition {
                at_rule: "media",
                condition: "not (min-width: 768px)".to_string(),
                priority: MAX_WIDTH_PRIORITY_BASE - 768,
            }
        );
        assert_eq!(
            parse("not-supports-[display:grid]"),
            Modifier::AtRuleCondition {
                at_rule: "supports",
                condition: "not (display:grid)".to_string(),
                priority: 66,
            }
        );
        // 伪元素无法取反
        assert!(err("not-before").contains("cannot be negated"));
    }
}
