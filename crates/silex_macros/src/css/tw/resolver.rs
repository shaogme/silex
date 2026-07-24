pub mod arbitrary;
pub mod numeric;
pub mod palette;
pub mod suggest;
pub mod table;
#[cfg(test)]
pub mod table_examples;

use crate::css::tw::ast::{Modifier, UtilityRule, UtilityValue};
use proc_macro2::Span;
use syn::{Error, Result};

pub(super) const RING_BOX_SHADOW: &str = "var(--tw-ring-inset, ) 0 0 0 var(--tw-ring-offset-width, 0px) var(--tw-ring-offset-color, #0000), 0 0 0 var(--tw-ring-width, 0px) var(--tw-ring-color, rgba(59, 130, 246, 0.5)), var(--tw-shadow, 0 0 #0000)";

pub(super) const DIVIDE_SELECTOR: &str = "& > :not([hidden]) ~ :not([hidden])";

#[inline]
pub(super) fn kw(s: &'static str) -> UtilityValue {
    UtilityValue::Keyword(s)
}

#[inline]
pub(super) fn num(v: f64, u: &'static str) -> UtilityValue {
    UtilityValue::Numeric(v, u)
}

#[inline]
pub(super) fn num_unitless(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "")
}

#[inline]
pub(super) fn rem(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "rem")
}

#[inline]
pub(super) fn px(v: f64) -> UtilityValue {
    UtilityValue::Numeric(v, "px")
}

#[inline]
pub(super) fn hex(s: &str) -> UtilityValue {
    UtilityValue::HexColor(s.to_string())
}

pub(super) fn make_rule(
    modifiers: Vec<Modifier>,
    prop: &str,
    value: UtilityValue,
    span: Span,
) -> UtilityRule {
    UtilityRule {
        modifiers,
        css_property: prop.to_string(),
        value,
        span,
    }
}

/// 判断是否为 Marker Class（如 group, peer, @container, container 或 group/name, peer/name, @container/name, container/name）
#[inline]
pub fn is_marker_class(token: &str) -> bool {
    let base = match token.split_once('/') {
        Some((prefix, _)) => prefix,
        None => token,
    };
    matches!(base, "group" | "peer" | "@container" | "container")
}

/// 将基础的 Utility 词条（如 `p-4`, `hover:bg-primary`, `w-[12px]`）解析为标准的 `UtilityRule`
pub fn resolve_utility(
    modifiers: Vec<Modifier>,
    utility_token: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let base = match utility_token.split_once('/') {
        Some((prefix, _)) => prefix,
        None => utility_token,
    };
    if matches!(base, "group" | "peer") {
        return Ok(vec![]);
    }

    // 1. 尝试匹配静态表规则 (static rules table)
    if let Some(rules) = table::resolve_static_rule(&modifiers, utility_token, span) {
        return Ok(rules);
    }

    // 2. 模式与规律型 Utility 解析
    resolve_pattern_utility(modifiers, utility_token, span)
}

#[inline]
fn color_prefix_to_prop(prefix: &str) -> Option<&'static str> {
    match prefix {
        "bg" => Some("background-color"),
        "text" => Some("color"),
        "border" => Some("border-color"),
        "border-t" => Some("border-top-color"),
        "border-r" => Some("border-right-color"),
        "border-b" => Some("border-bottom-color"),
        "border-l" => Some("border-left-color"),
        "outline" => Some("outline-color"),
        "ring" => Some("--tw-ring-color"),
        "ring-offset" => Some("--tw-ring-offset-color"),
        "from" => Some("--tw-gradient-from"),
        "via" => Some("--tw-gradient-via"),
        "to" => Some("--tw-gradient-to"),
        "divide" => Some("border-color"),
        "accent" => Some("accent-color"),
        "caret" => Some("caret-color"),
        "fill" => Some("fill"),
        "stroke" => Some("stroke"),
        _ => None,
    }
}

/// 解析前缀规律型 Utility (如 `p-4`, `mt-2`, `w-16`, `bg-theme(primary)`, `text-slate-900`, `bg-indigo-600/50`, `w-[12px]`)
fn resolve_pattern_utility(
    modifiers: Vec<Modifier>,
    token: &str,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    // 0. 处理命名容器 marker class (例: @container/card-header, container/sidebar)
    if let Some(c_name) = token
        .strip_prefix("@container/")
        .or_else(|| token.strip_prefix("container/"))
    {
        return Ok(vec![
            make_rule(
                modifiers.clone(),
                "container-name",
                UtilityValue::ArbitraryLiteral(c_name.to_string()),
                span,
            ),
            make_rule(modifiers, "container-type", kw("inline-size"), span),
        ]);
    }

    // 1. Theme 变量, 如 `bg-theme(primary)` / `text-theme(border)` / `bg-theme(primary/50)`
    if let Some((prefix, theme_var, opacity)) = parse_theme_var(token) {
        if prefix == "divide" {
            let c_mods = [
                modifiers.clone(),
                vec![Modifier::CustomSelector(DIVIDE_SELECTOR.into())],
            ]
            .concat();
            return Ok(vec![make_rule(
                c_mods,
                "border-color",
                UtilityValue::ThemeVar(theme_var.to_string(), opacity),
                span,
            )]);
        }
        if prefix == "ring" {
            return Ok(vec![
                make_rule(
                    modifiers.clone(),
                    "--tw-ring-color",
                    UtilityValue::ThemeVar(theme_var.to_string(), opacity),
                    span,
                ),
                make_rule(modifiers, "box-shadow", kw(RING_BOX_SHADOW), span),
            ]);
        }
        if prefix == "from" {
            return Ok(vec![
                make_rule(
                    modifiers.clone(),
                    "--tw-gradient-from",
                    UtilityValue::ThemeVar(theme_var.to_string(), opacity),
                    span,
                ),
                make_rule(
                    modifiers.clone(),
                    "--tw-gradient-to",
                    kw("rgb(255 255 255 / 0)"),
                    span,
                ),
                make_rule(
                    modifiers,
                    "--tw-gradient-stops",
                    kw("var(--tw-gradient-from), var(--tw-gradient-to)"),
                    span,
                ),
            ]);
        }
        if prefix == "via" {
            return Ok(vec![
                make_rule(
                    modifiers.clone(),
                    "--tw-gradient-via",
                    UtilityValue::ThemeVar(theme_var.to_string(), opacity),
                    span,
                ),
                make_rule(
                    modifiers,
                    "--tw-gradient-stops",
                    kw("var(--tw-gradient-from), var(--tw-gradient-via), var(--tw-gradient-to)"),
                    span,
                ),
            ]);
        }
        if let Some(prop) = color_prefix_to_prop(prefix) {
            return Ok(vec![make_rule(
                modifiers,
                prop,
                UtilityValue::ThemeVar(theme_var.to_string(), opacity),
                span,
            )]);
        }
        return Err(Error::new(
            span,
            format!("Unsupported theme prefix: '{}'", prefix),
        ));
    }

    // 2. Standard Palette 色系与零分配 Hex / /alpha 颜色换算
    if let Some((prop, val)) = palette::parse_color_utility(token) {
        return Ok(vec![make_rule(modifiers, prop, val, span)]);
    }

    // 3. Divide System: divide-x, divide-y, divide-x-2, divide-y-4, divide-solid, divide-dashed, divide-slate-200
    if let Some(rest) = token.strip_prefix("divide-") {
        let c_mods = [
            modifiers.clone(),
            vec![Modifier::CustomSelector(DIVIDE_SELECTOR.into())],
        ]
        .concat();

        match rest {
            "x" => {
                return Ok(vec![
                    make_rule(c_mods.clone(), "border-left-width", px(1.0), span),
                    make_rule(c_mods, "border-right-width", px(0.0), span),
                ]);
            }
            "y" => {
                return Ok(vec![
                    make_rule(c_mods.clone(), "border-top-width", px(1.0), span),
                    make_rule(c_mods, "border-bottom-width", px(0.0), span),
                ]);
            }
            "solid" => return Ok(vec![make_rule(c_mods, "border-style", kw("solid"), span)]),
            "dashed" => return Ok(vec![make_rule(c_mods, "border-style", kw("dashed"), span)]),
            "dotted" => return Ok(vec![make_rule(c_mods, "border-style", kw("dotted"), span)]),
            "none" => return Ok(vec![make_rule(c_mods, "border-style", kw("none"), span)]),
            _ => {}
        }

        if let Some(val_str) = rest.strip_prefix("x-") {
            if let Ok(n) = val_str.parse::<f64>() {
                return Ok(vec![
                    make_rule(c_mods.clone(), "border-left-width", px(n), span),
                    make_rule(c_mods, "border-right-width", px(0.0), span),
                ]);
            }
        } else if let Some(val_str) = rest.strip_prefix("y-")
            && let Ok(n) = val_str.parse::<f64>()
        {
            return Ok(vec![
                make_rule(c_mods.clone(), "border-top-width", px(n), span),
                make_rule(c_mods, "border-bottom-width", px(0.0), span),
            ]);
        }

        if let Some(color_val) = palette::parse_color_value(rest) {
            return Ok(vec![make_rule(c_mods, "border-color", color_val, span)]);
        }
    }

    // 4. Space System: space-x-2, space-y-4, -space-x-2, -space-y-4
    let is_negative = token.starts_with('-');
    let search_token = if is_negative { &token[1..] } else { token };

    if let Some(rest) = search_token.strip_prefix("space-") {
        let c_mods = [
            modifiers.clone(),
            vec![Modifier::CustomSelector(DIVIDE_SELECTOR.into())],
        ]
        .concat();

        if let Some(val_str) = rest.strip_prefix("x-") {
            if let Ok(n) = val_str.parse::<f64>() {
                let rem_val = n * if is_negative { -0.25 } else { 0.25 };
                return Ok(vec![
                    make_rule(c_mods.clone(), "margin-left", rem(rem_val), span),
                    make_rule(c_mods, "margin-right", px(0.0), span),
                ]);
            }
        } else if let Some(val_str) = rest.strip_prefix("y-")
            && let Ok(n) = val_str.parse::<f64>()
        {
            let rem_val = n * if is_negative { -0.25 } else { 0.25 };
            return Ok(vec![
                make_rule(c_mods.clone(), "margin-top", rem(rem_val), span),
                make_rule(c_mods, "margin-bottom", px(0.0), span),
            ]);
        }
    }

    // 5. Ring Colors: ring-offset-indigo-500, ring-indigo-500, ring-indigo-500/20
    if let Some(rest) = token.strip_prefix("ring-offset-") {
        if let Some(color_val) = palette::parse_color_value(rest) {
            return Ok(vec![
                make_rule(modifiers.clone(), "--tw-ring-offset-color", color_val, span),
                make_rule(modifiers, "box-shadow", kw(RING_BOX_SHADOW), span),
            ]);
        }
    } else if let Some(rest) = token.strip_prefix("ring-")
        && let Some(color_val) = palette::parse_color_value(rest)
    {
        return Ok(vec![
            make_rule(modifiers.clone(), "--tw-ring-color", color_val, span),
            make_rule(modifiers, "box-shadow", kw(RING_BOX_SHADOW), span),
        ]);
    }

    // 6. Gradient Stops: from-indigo-500, via-purple-500, to-pink-500
    if let Some(rest) = token.strip_prefix("from-") {
        if let Some(val) = palette::parse_color_value(rest) {
            return Ok(vec![
                make_rule(modifiers.clone(), "--tw-gradient-from", val, span),
                make_rule(
                    modifiers.clone(),
                    "--tw-gradient-to",
                    kw("rgb(255 255 255 / 0)"),
                    span,
                ),
                make_rule(
                    modifiers,
                    "--tw-gradient-stops",
                    kw("var(--tw-gradient-from), var(--tw-gradient-to)"),
                    span,
                ),
            ]);
        }
    } else if let Some(rest) = token.strip_prefix("via-") {
        if let Some(val) = palette::parse_color_value(rest) {
            return Ok(vec![
                make_rule(modifiers.clone(), "--tw-gradient-via", val, span),
                make_rule(
                    modifiers,
                    "--tw-gradient-stops",
                    kw("var(--tw-gradient-from), var(--tw-gradient-via), var(--tw-gradient-to)"),
                    span,
                ),
            ]);
        }
    } else if let Some(rest) = token.strip_prefix("to-")
        && let Some(val) = palette::parse_color_value(rest)
    {
        return Ok(vec![make_rule(modifiers, "--tw-gradient-to", val, span)]);
    }

    // 7. Grid Spans & Line Clamp: col-span-2, col-start-3, col-end-4, row-span-2, line-clamp-2
    if let Some(rest) = token.strip_prefix("col-span-") {
        if let Ok(n) = rest.parse::<usize>() {
            return Ok(vec![make_rule(
                modifiers,
                "grid-column",
                UtilityValue::ArbitraryLiteral(format!("span {} / span {}", n, n)),
                span,
            )]);
        }
    } else if let Some(rest) = token.strip_prefix("col-start-") {
        if let Ok(n) = rest.parse::<f64>() {
            return Ok(vec![make_rule(
                modifiers,
                "grid-column-start",
                num_unitless(n),
                span,
            )]);
        }
    } else if let Some(rest) = token.strip_prefix("col-end-") {
        if let Ok(n) = rest.parse::<f64>() {
            return Ok(vec![make_rule(
                modifiers,
                "grid-column-end",
                num_unitless(n),
                span,
            )]);
        }
    } else if let Some(rest) = token.strip_prefix("row-span-") {
        if let Ok(n) = rest.parse::<usize>() {
            return Ok(vec![make_rule(
                modifiers,
                "grid-row",
                UtilityValue::ArbitraryLiteral(format!("span {} / span {}", n, n)),
                span,
            )]);
        }
    } else if let Some(rest) = token.strip_prefix("row-start-") {
        if let Ok(n) = rest.parse::<f64>() {
            return Ok(vec![make_rule(
                modifiers,
                "grid-row-start",
                num_unitless(n),
                span,
            )]);
        }
    } else if let Some(rest) = token.strip_prefix("row-end-") {
        if let Ok(n) = rest.parse::<f64>() {
            return Ok(vec![make_rule(
                modifiers,
                "grid-row-end",
                num_unitless(n),
                span,
            )]);
        }
    } else if let Some(rest) = token.strip_prefix("line-clamp-")
        && let Ok(n) = rest.parse::<f64>()
    {
        return Ok(vec![
            make_rule(modifiers.clone(), "overflow", kw("hidden"), span),
            make_rule(modifiers.clone(), "display", kw("-webkit-box"), span),
            make_rule(
                modifiers.clone(),
                "-webkit-box-orient",
                kw("vertical"),
                span,
            ),
            make_rule(modifiers, "-webkit-line-clamp", num_unitless(n), span),
        ]);
    }

    // 8. 任意值与动态表达式语法, 如 `w-[100px]` 或 `p-[$(pad_val)]`
    if let Some((prefix, raw_val)) = arbitrary::parse_arbitrary_syntax(token) {
        return arbitrary::resolve_arbitrary(modifiers, prefix, raw_val, span);
    }

    // 9. 数值、分数 (1/2, 1/3) 与方向边距/定位 Utility 解析
    if let Some(rules) = numeric::resolve_numeric_utility(&modifiers, token, span) {
        return Ok(rules);
    }

    // 10. Levenshtein 智能纠错与建议
    let suggestion = suggest::find_best_suggestion(token);
    let msg = match suggestion {
        Some(s) => format!(
            "Unknown or unsupported Utility class '{}'. Did you mean '{}'?",
            token, s
        ),
        None => format!("Unknown or unsupported Utility class '{}'.", token),
    };

    Err(Error::new(span, msg))
}

fn parse_theme_var(token: &str) -> Option<(&str, &str, Option<f64>)> {
    if let Some((prefix, rest)) = token.split_once("-theme(")
        && let Some(inner) = rest.strip_suffix(')')
    {
        if let Some((var_name, op_str)) = inner.split_once('/')
            && let Ok(op) = op_str.parse::<f64>()
        {
            return Some((prefix, var_name, Some(op)));
        }
        return Some((prefix, inner, None));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    #[test]
    fn test_resolve_pattern_numeric_rules() {
        let span = Span::call_site();

        // 1. 单属性规则 (rem 缩放)
        let rules = resolve_utility(vec![], "p-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "padding");
        assert_eq!(rules[0].value, UtilityValue::Numeric(1.0, "rem"));

        let rules = resolve_utility(vec![], "-mt-2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "margin-top");
        assert_eq!(rules[0].value, UtilityValue::Numeric(-0.5, "rem"));

        // 2. 双属性规则 (对称方向)
        let rules = resolve_utility(vec![], "px-6", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "padding-left");
        assert_eq!(rules[1].css_property, "padding-right");
        assert_eq!(rules[0].value, UtilityValue::Numeric(1.5, "rem"));
        assert_eq!(rules[1].value, UtilityValue::Numeric(1.5, "rem"));

        let rules = resolve_utility(vec![], "size-8", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(rules[1].css_property, "height");
        assert_eq!(rules[0].value, UtilityValue::Numeric(2.0, "rem"));

        // 3. 自定义数值计算与转换规则
        let rules = resolve_utility(vec![], "grid-cols-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "grid-template-columns");
        assert_eq!(
            rules[0].value,
            UtilityValue::Keyword("repeat(4, minmax(0, 1fr))")
        );

        let rules = resolve_utility(vec![], "opacity-50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "opacity");
        assert_eq!(rules[0].value, UtilityValue::Numeric(0.5, ""));

        let rules = resolve_utility(vec![], "rotate-45", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(rules[0].value, UtilityValue::Keyword("rotate(45deg)"));

        let rules = resolve_utility(vec![], "-rotate-90", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(rules[0].value, UtilityValue::Keyword("rotate(-90deg)"));

        let rules = resolve_utility(vec![], "bg-theme(primary)", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ThemeVar("primary".into(), None)
        );

        let rules = resolve_utility(vec![], "bg-theme(primary/50)", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ThemeVar("primary".into(), Some(50.0))
        );

        // 4. Hex 颜色解析规则
        let rules = resolve_utility(vec![], "bg-[#1e293b]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#1e293b".into()));

        let rules = resolve_utility(vec![], "text-[#818cf8]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#818cf8".into()));

        // 5. 通用任意值语法解析规则
        let rules = resolve_utility(vec![], "w-[100px]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("100px".into())
        );

        // 6. Levenshtein 拼写纠错测试
        let err = resolve_utility(vec![], "flexx", span).unwrap_err();
        assert!(err.to_string().contains("Did you mean 'flex'?"));

        let err = resolve_utility(vec![], "items-centerr", span).unwrap_err();
        assert!(err.to_string().contains("Did you mean 'items-center'?"));

        // 7. Phase 4: Container Query Utilities
        let rules = resolve_utility(vec![], "@container", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "container-type");
        assert_eq!(rules[0].value, UtilityValue::Keyword("inline-size"));

        let rules = resolve_utility(vec![], "container-[sidebar]", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "container-name");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("sidebar".into())
        );

        // 8. Phase 5: Standard Color Palette & Opacity Suffix Rules
        let rules = resolve_utility(vec![], "text-slate-900", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#0f172a".into()));

        let rules = resolve_utility(vec![], "bg-indigo-600/50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rgba(79, 70, 229, 0.5)".into())
        );

        let rules = resolve_utility(vec![], "border-emerald-500/25", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rgba(16, 185, 129, 0.25)".into())
        );

        let rules = resolve_utility(vec![], "border-t-rose-500", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-top-color");
        assert_eq!(rules[0].value, UtilityValue::HexColor("#f43f5e".into()));

        let rules = resolve_utility(vec![], "bg-white/50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rgba(255, 255, 255, 0.5)".into())
        );
    }

    #[test]
    fn test_new_fractional_and_directional_features() {
        let span = Span::call_site();

        // 分数宽度测试
        let rules = resolve_utility(vec![], "w-1/2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "width");
        assert_eq!(rules[0].value, UtilityValue::Numeric(50.0, "%"));

        let rules = resolve_utility(vec![], "h-1/3", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "height");
        if let UtilityValue::Numeric(val, unit) = &rules[0].value {
            assert_eq!(*unit, "%");
            assert!((val - 33.333333333333336).abs() < 1e-6);
        } else {
            panic!("Expected Numeric");
        }

        // 分数 translate
        let rules = resolve_utility(vec![], "-translate-x-1/2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "transform");
        assert_eq!(rules[0].value, UtilityValue::Keyword("translateX(-50%)"));

        // 定位与负 inset
        let rules = resolve_utility(vec![], "-top-4", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "top");
        assert_eq!(rules[0].value, UtilityValue::Numeric(-1.0, "rem"));

        let rules = resolve_utility(vec![], "inset-x-0", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "left");
        assert_eq!(rules[1].css_property, "right");

        // 方向 Border 宽度
        let rules = resolve_utility(vec![], "border-t-2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-top-width");
        assert_eq!(rules[0].value, UtilityValue::Numeric(2.0, "px"));

        let rules = resolve_utility(vec![], "border-x-4", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "border-left-width");
        assert_eq!(rules[1].css_property, "border-right-width");

        // 完全透明度 /0 测试
        let rules = resolve_utility(vec![], "bg-black/0", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("rgba(0, 0, 0, 0)".into())
        );

        // 新扩充静态词条测试
        let rules = resolve_utility(vec![], "max-w-xs", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "max-width");
        assert_eq!(rules[0].value, UtilityValue::Numeric(20.0, "rem"));

        let rules = resolve_utility(vec![], "text-5xl", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "font-size");
        assert_eq!(rules[0].value, UtilityValue::Numeric(3.0, "rem"));

        let rules = resolve_utility(vec![], "italic", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "font-style");
        assert_eq!(rules[0].value, UtilityValue::Keyword("italic"));

        // 多列与分栏 Break 规则测试
        let rules = resolve_utility(vec![], "columns-4xl", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "column-width");
        assert_eq!(rules[0].value, UtilityValue::Numeric(56.0, "rem"));

        let rules = resolve_utility(vec![], "columns-2", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "column-count");
        assert_eq!(rules[0].value, UtilityValue::Numeric(2.0, ""));

        let rules = resolve_utility(vec![], "break-inside-avoid-flex", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "break-inside");
        assert_eq!(rules[0].value, UtilityValue::Keyword("avoid-flex"));

        // z-index, box-decoration & isolation 规则测试
        let rules = resolve_utility(vec![], "z-50", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "z-index");
        assert_eq!(rules[0].value, UtilityValue::Numeric(50.0, ""));

        let rules = resolve_utility(vec![], "-z-10", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "z-index");
        assert_eq!(rules[0].value, UtilityValue::Numeric(-10.0, ""));

        let rules = resolve_utility(vec![], "z-auto", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "z-index");
        assert_eq!(rules[0].value, UtilityValue::Keyword("auto"));

        let rules = resolve_utility(vec![], "box-decoration-slice", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "box-decoration-break");
        assert_eq!(rules[0].value, UtilityValue::Keyword("slice"));

        let rules = resolve_utility(vec![], "isolate", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "isolation");
        assert_eq!(rules[0].value, UtilityValue::Keyword("isolate"));

        // Outline 规则测试 (outline-1, outline-ring)
        let rules = resolve_utility(vec![], "outline-1", span).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].css_property, "outline-style");
        assert_eq!(rules[0].value, UtilityValue::Keyword("solid"));
        assert_eq!(rules[1].css_property, "outline-width");
        assert_eq!(rules[1].value, UtilityValue::Numeric(1.0, "px"));

        let rules = resolve_utility(vec![], "outline-ring", span).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "outline-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("var(--ring)".into())
        );
    }
}
