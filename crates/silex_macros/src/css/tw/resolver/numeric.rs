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

/// 将求值结果渲染为可嵌入 `value_wrapper` 的 CSS 字面量
fn utility_value_to_literal(val: &UtilityValue) -> String {
    match val {
        UtilityValue::Numeric(v, unit) => format!("{}{}", format_num_clean(*v), unit),
        UtilityValue::Keyword(k) => (*k).to_string(),
        UtilityValue::HexColor(hex) => hex.clone(),
        UtilityValue::ArbitraryLiteral(s) => s.clone(),
        UtilityValue::ThemeVar(var, opacity) => match opacity {
            Some(op) => format!(
                "color-mix(in srgb, var(--slx-theme-{}) {}%, transparent)",
                var, op
            ),
            None => format!("var(--slx-theme-{})", var),
        },
        UtilityValue::DynamicExpr(expr, _) => quote::quote!(#expr).to_string(),
    }
}

/// 解析数值与分数开头的规则 (例: `p-4`, `w-1/2`, `-top-4`, `border-t-2`)
///
/// 注意这里**不再**处理 `rounded-*`：圆角档位由 `silex_tw_core` 统一解析。
/// 此前本模块自带一份档位表，`sm` 停留在 Tailwind v3 的 `0.125rem`，
/// 而 core 侧早已修正为 v4 的 `0.25rem`——只因静态表优先命中才一直没暴露（报告 §3.1）。
pub fn resolve_numeric_utility(
    modifiers: &[SpannedModifier],
    token: &str,
    span: Span,
) -> Option<Vec<UtilityRule>> {
    let is_negative = token.starts_with('-');
    let search_token = if is_negative { &token[1..] } else { token };

    let (prefix, val_str) = search_token.rsplit_once('-')?;
    let (val_num, is_fraction) = parse_num_or_fraction(val_str)?;

    let meta = lookup_prefix_meta(prefix)?;
    let mods = modifiers.to_vec();
    let sign = if is_negative { -1.0 } else { 1.0 };

    // 1. 通用单位元数据求值 (Generic Unit Evaluation)
    let val = if is_fraction {
        num(val_num * 100.0 * sign, "%")
    } else {
        match meta.unit_kind {
            UnitKind::RemScale => rem(val_num * 0.25 * sign),
            UnitKind::Pixel => px(val_num * sign),
            UnitKind::Percentage => num_unitless(val_num / 100.0 * sign),
            UnitKind::Milliseconds => num(val_num, "ms"),
            UnitKind::Unitless => num_unitless(val_num * sign),
            UnitKind::Degree => {
                UtilityValue::ArbitraryLiteral(format!("{}deg", format_num_clean(val_num * sign)))
            }
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

    // 2. 应用元数据声明的 value_wrapper（如 `transform: rotate({})`、`filter: blur({})`、
    //    `slide-in-from-top` 的取反 `-{}`）。这是 wrapper 的唯一消费点，
    //    禁止再为个别前缀写硬编码特判。
    let val = match meta.value_wrapper {
        Some(wrapper) => {
            UtilityValue::ArbitraryLiteral(wrapper.replace("{}", &utility_value_to_literal(&val)))
        }
        None => val,
    };

    // 3. 伴生声明（`outline-*` 的 `outline-style: solid`）同样来自元数据
    let rules = meta
        .target_props
        .iter()
        .map(|&p| make_rule(mods.clone(), p, val.clone(), span))
        .chain(
            meta.companions
                .iter()
                .map(|&(p, v)| make_rule(mods.clone(), p, kw(v), span)),
        )
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
