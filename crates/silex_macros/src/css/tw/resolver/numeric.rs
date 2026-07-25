use crate::css::tw::ast::{SpannedModifier, UtilityRule, UtilityValue};
use proc_macro2::Span;

use syn::Result;

use super::{kw, make_rule, num, num_unitless, px, rem};

use super::codegen::prefix_metadata::{UnitKind, lookup_prefix_meta};

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
    modifiers: &[SpannedModifier],
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
        .collect::<Result<Vec<_>>>()
        .ok()?;
    Some(rules)
}

/// 解析数值与分数开头的规则 (例: `p-4`, `w-1/2`, `-top-4`, `border-t-2`, `rounded-tr-lg`)
pub fn resolve_numeric_utility(
    modifiers: &[SpannedModifier],
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

    let meta = lookup_prefix_meta(prefix)?;
    let mods = modifiers.to_vec();
    let sign = if is_negative { -1.0 } else { 1.0 };

    // 1. Outline 特殊逻辑 (需追加 outline-style: solid)
    if prefix == "outline" && !is_fraction {
        return Some(vec![
            make_rule(mods.clone(), "outline-style", kw("solid"), span).ok()?,
            make_rule(mods, "outline-width", px(val_num), span).ok()?,
        ]);
    }

    // 2. Transform 变体 (rotate, scale, translate, skew)
    if prefix == "rotate" {
        return Some(vec![
            make_rule(
                mods,
                "transform",
                UtilityValue::ArbitraryLiteral(format!(
                    "rotate({}deg)",
                    if is_negative { -val_num } else { val_num }
                )),
                span,
            )
            .ok()?,
        ]);
    }
    if prefix == "scale" || prefix == "scale-x" || prefix == "scale-y" {
        let fn_name = match prefix {
            "scale-x" => "scaleX",
            "scale-y" => "scaleY",
            _ => "scale",
        };
        return Some(vec![
            make_rule(
                mods,
                "transform",
                UtilityValue::ArbitraryLiteral(format!(
                    "{}({})",
                    fn_name,
                    format_num_clean(val_num / 100.0)
                )),
                span,
            )
            .ok()?,
        ]);
    }
    if prefix == "skew-x" || prefix == "skew-y" {
        let fn_name = if prefix == "skew-x" { "skewX" } else { "skewY" };
        return Some(vec![
            make_rule(
                mods,
                "transform",
                UtilityValue::ArbitraryLiteral(format!(
                    "{}({}deg)",
                    fn_name,
                    if is_negative { -val_num } else { val_num }
                )),
                span,
            )
            .ok()?,
        ]);
    }
    if prefix == "translate-x" || prefix == "translate-y" {
        let fn_name = if prefix == "translate-x" {
            "translateX"
        } else {
            "translateY"
        };
        let val_repr = if is_fraction {
            format!("{}%", format_num_clean(val_num * 100.0 * sign))
        } else if val_num == 0.0 {
            "0px".to_string()
        } else {
            format!("{}rem", format_num_clean(val_num * 0.25 * sign))
        };
        return Some(vec![
            make_rule(
                mods,
                "transform",
                UtilityValue::ArbitraryLiteral(format!("{}({})", fn_name, val_repr)),
                span,
            )
            .ok()?,
        ]);
    }
    if prefix.starts_with("slide-in-from-") {
        let rem_val = val_num * 0.25;
        let val_repr = if prefix.contains("-top") || prefix.contains("-left") {
            format!("-{}rem", format_num_clean(rem_val))
        } else {
            format!("{}rem", format_num_clean(rem_val))
        };
        return Some(vec![
            make_rule(
                mods,
                meta.target_props[0],
                UtilityValue::ArbitraryLiteral(val_repr),
                span,
            )
            .ok()?,
        ]);
    }

    // 3. 通用单位元数据求值 (Generic Unit Evaluation)
    let val = if is_fraction {
        num(val_num * 100.0 * sign, "%")
    } else {
        match meta.unit_kind {
            UnitKind::RemScale => rem(val_num * 0.25 * sign),
            UnitKind::Pixel => px(val_num * sign),
            UnitKind::Percentage => num_unitless(val_num / 100.0),
            UnitKind::Milliseconds => num(val_num, "ms"),
            UnitKind::Unitless => num_unitless(val_num * sign),
            UnitKind::Degree => UtilityValue::ArbitraryLiteral(format!(
                "{}deg",
                if is_negative { -val_num } else { val_num }
            )),
            UnitKind::GridRepeat => UtilityValue::ArbitraryLiteral(format!(
                "repeat({}, minmax(0, 1fr))",
                val_num as usize
            )),
            UnitKind::GridSpan => UtilityValue::ArbitraryLiteral(format!(
                "span {} / span {}",
                val_num as usize, val_num as usize
            )),
        }
    };

    let rules = meta
        .target_props
        .iter()
        .map(|&p| make_rule(mods.clone(), p, val.clone(), span))
        .collect::<Result<Vec<_>>>()
        .ok()?;

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
