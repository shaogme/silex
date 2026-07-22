use crate::css::tw::ast::{Modifier, UtilityRule, UtilityValue};
use proc_macro2::Span;

use super::{make_rule, num, num_unitless, px, rem};

#[inline]
fn format_num_clean(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{:.0}", v)
    } else {
        format!("{:.6}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// 解析方向与各角 Rounded 规则 (例: `rounded-tl-3xl`, `rounded-t-lg`, `rounded-tr-4`)
pub fn resolve_rounded_utility(
    modifiers: &[Modifier],
    token: &str,
    span: Span,
) -> Option<Vec<UtilityRule>> {
    let rest = token.strip_prefix("rounded-")?;

    let (props, size_str): (&[&str], &str) = if let Some(s) = rest.strip_prefix("tl-") {
        (&["border-top-left-radius"], s)
    } else if let Some(s) = rest.strip_prefix("tr-") {
        (&["border-top-right-radius"], s)
    } else if let Some(s) = rest.strip_prefix("br-") {
        (&["border-bottom-right-radius"], s)
    } else if let Some(s) = rest.strip_prefix("bl-") {
        (&["border-bottom-left-radius"], s)
    } else if let Some(s) = rest.strip_prefix("t-") {
        (&["border-top-left-radius", "border-top-right-radius"], s)
    } else if let Some(s) = rest.strip_prefix("r-") {
        (
            &["border-top-right-radius", "border-bottom-right-radius"],
            s,
        )
    } else if let Some(s) = rest.strip_prefix("b-") {
        (
            &["border-bottom-right-radius", "border-bottom-left-radius"],
            s,
        )
    } else {
        let s = rest.strip_prefix("l-")?;
        (&["border-top-left-radius", "border-bottom-left-radius"], s)
    };

    let val = match size_str {
        "none" => px(0.0),
        "sm" => rem(0.125),
        "" | "md" => rem(0.375),
        "lg" => rem(0.5),
        "xl" => rem(0.75),
        "2xl" => rem(1.0),
        "3xl" => rem(1.5),
        "full" => px(9999.0),
        s => {
            let n: f64 = s.parse().ok()?;
            rem(n * 0.25)
        }
    };

    let mods = modifiers.to_vec();
    let rules = props
        .iter()
        .map(|p| make_rule(mods.clone(), p, val.clone(), span))
        .collect();
    Some(rules)
}

/// 解析数值与分数开头的规则 (例: `p-4`, `w-1/2`, `-top-4`, `border-t-2`, `rounded-tr-lg`)
pub fn resolve_numeric_utility(
    modifiers: &[Modifier],
    token: &str,
    span: Span,
) -> Option<Vec<UtilityRule>> {
    if let Some(rules) = resolve_rounded_utility(modifiers, token, span) {
        return Some(rules);
    }

    let is_negative = token.starts_with('-');
    let search_token = if is_negative { &token[1..] } else { token };

    let (prefix, val_str) = search_token.rsplit_once('-')?;
    let (val_num, is_fraction) = parse_num_or_fraction(val_str)?;

    let mods = modifiers.to_vec();

    // 1. 方向边框宽度 (border-t-2, border-x-4, border-2 等)
    if prefix.starts_with("border") && !is_fraction {
        let border_rules = match prefix {
            "border" => vec![make_rule(mods, "border-width", px(val_num), span)],
            "border-t" => vec![make_rule(mods, "border-top-width", px(val_num), span)],
            "border-r" => vec![make_rule(mods, "border-right-width", px(val_num), span)],
            "border-b" => vec![make_rule(mods, "border-bottom-width", px(val_num), span)],
            "border-l" => vec![make_rule(mods, "border-left-width", px(val_num), span)],
            "border-x" => vec![
                make_rule(mods.clone(), "border-left-width", px(val_num), span),
                make_rule(mods, "border-right-width", px(val_num), span),
            ],
            "border-y" => vec![
                make_rule(mods.clone(), "border-top-width", px(val_num), span),
                make_rule(mods, "border-bottom-width", px(val_num), span),
            ],
            _ => return None,
        };
        return Some(border_rules);
    }

    // 1.5 文本下划线偏移 (underline-offset-4, underline-offset-2 等)
    if prefix == "underline-offset" && !is_fraction {
        let px_val = if is_negative { -val_num } else { val_num };
        return Some(vec![make_rule(
            mods,
            "text-underline-offset",
            px(px_val),
            span,
        )]);
    }

    // 2. 定位与 Inset 系统 (top-4, -left-1/2, inset-x-0 等)
    if matches!(
        prefix,
        "top" | "bottom" | "left" | "right" | "inset" | "inset-x" | "inset-y"
    ) {
        let sign = if is_negative { -1.0 } else { 1.0 };
        let final_val = if is_fraction {
            num(val_num * 100.0 * sign, "%")
        } else {
            rem(val_num * 0.25 * sign)
        };

        let pos_rules = match prefix {
            "top" => vec![make_rule(mods, "top", final_val, span)],
            "bottom" => vec![make_rule(mods, "bottom", final_val, span)],
            "left" => vec![make_rule(mods, "left", final_val, span)],
            "right" => vec![make_rule(mods, "right", final_val, span)],
            "inset" => vec![
                make_rule(mods.clone(), "top", final_val.clone(), span),
                make_rule(mods.clone(), "right", final_val.clone(), span),
                make_rule(mods.clone(), "bottom", final_val.clone(), span),
                make_rule(mods, "left", final_val, span),
            ],
            "inset-x" => vec![
                make_rule(mods.clone(), "left", final_val.clone(), span),
                make_rule(mods, "right", final_val, span),
            ],
            "inset-y" => vec![
                make_rule(mods.clone(), "top", final_val.clone(), span),
                make_rule(mods, "bottom", final_val, span),
            ],
            _ => unreachable!(),
        };
        return Some(pos_rules);
    }

    // 3. 边距、尺寸与间距规则
    let scale = if is_negative { -0.25 } else { 0.25 };
    let rem_val = val_num * scale;
    let rem_rule_val = rem(rem_val);

    if is_fraction {
        let pct_sign = if is_negative { -100.0 } else { 100.0 };
        let pct_val = num(val_num * pct_sign, "%");

        let frac_rules = match prefix {
            "w" => vec![make_rule(mods, "width", pct_val, span)],
            "h" => vec![make_rule(mods, "height", pct_val, span)],
            "min-w" => vec![make_rule(mods, "min-width", pct_val, span)],
            "min-h" => vec![make_rule(mods, "min-height", pct_val, span)],
            "max-w" => vec![make_rule(mods, "max-width", pct_val, span)],
            "max-h" => vec![make_rule(mods, "max-height", pct_val, span)],
            "size" => vec![
                make_rule(mods.clone(), "width", pct_val.clone(), span),
                make_rule(mods, "height", pct_val, span),
            ],
            "translate-x" => vec![make_rule(
                mods,
                "transform",
                UtilityValue::ArbitraryLiteral(format!(
                    "translateX({}%)",
                    format_num_clean(val_num * pct_sign)
                )),
                span,
            )],
            "translate-y" => vec![make_rule(
                mods,
                "transform",
                UtilityValue::ArbitraryLiteral(format!(
                    "translateY({}%)",
                    format_num_clean(val_num * pct_sign)
                )),
                span,
            )],
            _ => return None,
        };
        return Some(frac_rules);
    }

    // 4. 标准数值规则映射
    let rules = match prefix {
        // 单属性边距 / 尺寸
        "p" => vec![make_rule(mods, "padding", rem_rule_val, span)],
        "pt" => vec![make_rule(mods, "padding-top", rem_rule_val, span)],
        "pr" => vec![make_rule(mods, "padding-right", rem_rule_val, span)],
        "pb" => vec![make_rule(mods, "padding-bottom", rem_rule_val, span)],
        "pl" => vec![make_rule(mods, "padding-left", rem_rule_val, span)],
        "m" => vec![make_rule(mods, "margin", rem_rule_val, span)],
        "mt" => vec![make_rule(mods, "margin-top", rem_rule_val, span)],
        "mr" => vec![make_rule(mods, "margin-right", rem_rule_val, span)],
        "mb" => vec![make_rule(mods, "margin-bottom", rem_rule_val, span)],
        "ml" => vec![make_rule(mods, "margin-left", rem_rule_val, span)],
        "gap" => vec![make_rule(mods, "gap", rem_rule_val, span)],
        "gap-x" => vec![make_rule(mods, "column-gap", rem_rule_val, span)],
        "gap-y" => vec![make_rule(mods, "row-gap", rem_rule_val, span)],
        "w" => vec![make_rule(mods, "width", rem_rule_val, span)],
        "h" => vec![make_rule(mods, "height", rem_rule_val, span)],
        "min-w" => vec![make_rule(mods, "min-width", rem_rule_val, span)],
        "min-h" => vec![make_rule(mods, "min-height", rem_rule_val, span)],
        "max-w" => vec![make_rule(mods, "max-width", rem_rule_val, span)],
        "max-h" => vec![make_rule(mods, "max-height", rem_rule_val, span)],

        // 对称双属性映射
        "px" => vec![
            make_rule(mods.clone(), "padding-left", rem_rule_val.clone(), span),
            make_rule(mods, "padding-right", rem_rule_val, span),
        ],
        "py" => vec![
            make_rule(mods.clone(), "padding-top", rem_rule_val.clone(), span),
            make_rule(mods, "padding-bottom", rem_rule_val, span),
        ],
        "mx" => vec![
            make_rule(mods.clone(), "margin-left", rem_rule_val.clone(), span),
            make_rule(mods, "margin-right", rem_rule_val, span),
        ],
        "my" => vec![
            make_rule(mods.clone(), "margin-top", rem_rule_val.clone(), span),
            make_rule(mods, "margin-bottom", rem_rule_val, span),
        ],
        "size" => vec![
            make_rule(mods.clone(), "width", rem_rule_val.clone(), span),
            make_rule(mods, "height", rem_rule_val, span),
        ],

        // 复杂与转换规则
        "z" => vec![make_rule(
            mods,
            "z-index",
            num_unitless(if is_negative { -val_num } else { val_num }),
            span,
        )],
        "columns" => vec![make_rule(mods, "column-count", num_unitless(val_num), span)],
        "grid-cols" => vec![make_rule(
            mods,
            "grid-template-columns",
            UtilityValue::ArbitraryLiteral(format!("repeat({}, minmax(0, 1fr))", val_num as usize)),
            span,
        )],
        "grid-rows" => vec![make_rule(
            mods,
            "grid-template-rows",
            UtilityValue::ArbitraryLiteral(format!("repeat({}, minmax(0, 1fr))", val_num as usize)),
            span,
        )],
        "opacity" => vec![make_rule(
            mods,
            "opacity",
            num_unitless(val_num / 100.0),
            span,
        )],
        "duration" => vec![make_rule(
            mods,
            "transition-duration",
            num(val_num, "ms"),
            span,
        )],
        "rotate" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!(
                "rotate({}deg)",
                if is_negative { -val_num } else { val_num }
            )),
            span,
        )],
        "scale" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!("scale({})", format_num_clean(val_num / 100.0))),
            span,
        )],
        "scale-x" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!(
                "scaleX({})",
                format_num_clean(val_num / 100.0)
            )),
            span,
        )],
        "scale-y" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!(
                "scaleY({})",
                format_num_clean(val_num / 100.0)
            )),
            span,
        )],
        "skew-x" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!(
                "skewX({}deg)",
                if is_negative { -val_num } else { val_num }
            )),
            span,
        )],
        "skew-y" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!(
                "skewY({}deg)",
                if is_negative { -val_num } else { val_num }
            )),
            span,
        )],
        "translate-x" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!("translateX({}rem)", format_num_clean(rem_val))),
            span,
        )],
        "translate-y" => vec![make_rule(
            mods,
            "transform",
            UtilityValue::ArbitraryLiteral(format!("translateY({}rem)", format_num_clean(rem_val))),
            span,
        )],
        _ => return None,
    };

    Some(rules)
}

/// 解析纯数字或分数表达式（如 `4`, `1.5`, `1/2`, `3/4`）
fn parse_num_or_fraction(s: &str) -> Option<(f64, bool)> {
    if let Some((num_str, den_str)) = s.split_once('/') {
        let num: f64 = num_str.parse().ok()?;
        let den: f64 = den_str.parse().ok()?;
        if den == 0.0 {
            return None;
        }
        Some((num / den, true))
    } else {
        let val: f64 = s.parse().ok()?;
        Some((val, false))
    }
}
