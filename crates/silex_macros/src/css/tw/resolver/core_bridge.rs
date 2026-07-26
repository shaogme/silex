//! `silex_tw_core` 与 proc-macro 世界之间的适配层。
//!
//! core 产出的是宿主无关的 `(属性名, CSS 值文本)`；宏这边需要的是带 span、
//! 带修饰符、值已归类成 `UtilityValue` 的 `UtilityRule`。转换全部收在这里，
//! 解析规则本身一行都不重复——那正是报告 §3.1 要根治的东西。

use std::borrow::Cow;

use proc_macro2::Span;
use silex_tw_core::{TwRuleSet, TwValueKind, classify};
use syn::Result;

use crate::css::tw::{
    ast::{Modifier, SpannedModifier, UtilityRule, UtilityValue},
    resolver::{codegen::palette, make_rule},
};

/// 宏侧的解析上下文：色板取自生成的静态表，再叠加用户 `silex.toml` 的自定义颜色。
pub struct MacroCtx;

impl silex_tw_core::TwContext for MacroCtx {
    fn config_color(&self, name: &str) -> Option<String> {
        let cfg = crate::css::config::get_config()?;
        cfg.theme
            .colors
            .get(name)
            .or_else(|| cfg.theme.dark_colors.get(name))
            .cloned()
    }

    fn palette_shade(&self, family: &str, shade: &str) -> Option<&str> {
        palette::lookup_palette_color_fast(family, shade)
    }

    fn palette_ramp(&self, family: &str) -> Option<[&str; 11]> {
        palette::get_raw_palette(family)
    }
}

/// 把 core 的值文本归类成 `UtilityValue`。
///
/// 判定走 core 的 [`classify`]——生成静态表时用的也是它，
/// 因此"查表命中"与"模式解析兜底"两条路径对同一个值不可能给出不同的变体。
pub fn to_utility_value(value: Cow<'static, str>) -> UtilityValue {
    match classify(&value) {
        TwValueKind::Numeric(v, unit) => UtilityValue::Numeric(v, unit),
        TwValueKind::Hex => UtilityValue::HexColor(value.into_owned()),
        TwValueKind::Literal => UtilityValue::ArbitraryLiteral(value.into_owned()),
        TwValueKind::RingShadow => UtilityValue::Keyword(super::RING_BOX_SHADOW),
        // 借用态的关键字可以零拷贝地保留成 `&'static str`
        TwValueKind::Keyword => match value {
            Cow::Borrowed(s) => UtilityValue::Keyword(s),
            Cow::Owned(s) => UtilityValue::ArbitraryLiteral(s),
        },
    }
}

/// 把伴生选择器挂成一个额外的修饰符
pub fn with_selector(
    modifiers: &[SpannedModifier],
    selector: Option<&'static str>,
    span: Span,
) -> Vec<SpannedModifier> {
    let mut mods = modifiers.to_vec();
    if let Some(sel) = selector {
        mods.push(SpannedModifier::new(
            Modifier::CustomSelector(sel.to_string()),
            span,
        ));
    }
    mods
}

/// 把 at-rule 条件挂成一个额外的修饰符
fn with_media(
    modifiers: &[SpannedModifier],
    condition: &str,
    priority: u32,
    span: Span,
) -> Vec<SpannedModifier> {
    let mut mods = modifiers.to_vec();
    mods.push(SpannedModifier::new(
        Modifier::AtRuleCondition {
            at_rule: "media",
            condition: condition.to_string(),
            priority,
        },
        span,
    ));
    mods
}

/// 非宽度类媒体特性的权重，与 [`Modifier::MediaQuery`] 同级
const MEDIA_FEATURE_PRIORITY: u32 = 65;

/// `container` 的档位：core 里的默认五档 + `silex.toml` 里额外配置的断点。
///
/// 额外断点按宽度升序追加，与默认档位一起构成一条单调递增的链——
/// 顺序不能依赖 `HashMap` 的迭代顺序，那会让类名哈希不可复现（§11.5 踩过一次）。
fn container_tiers() -> Vec<(String, u32)> {
    let mut tiers: Vec<(String, u32)> = silex_tw_core::CONTAINER_TIERS
        .iter()
        .map(|(_, width)| ((*width).to_string(), css_length_px(width)))
        .collect();

    if let Some(cfg) = crate::css::config::get_config() {
        let builtin: std::collections::BTreeSet<&str> = silex_tw_core::CONTAINER_TIERS
            .iter()
            .map(|(name, _)| *name)
            .collect();
        let mut extra: Vec<(String, u32)> = cfg
            .theme
            .breakpoints
            .iter()
            .filter(|(name, _)| !builtin.contains(name.as_str()))
            .map(|(_, width)| (width.clone(), css_length_px(width)))
            .collect();
        extra.sort();
        tiers.extend(extra);
    }

    tiers.sort_by_key(|(_, px)| *px);
    tiers.dedup_by(|a, b| a.1 == b.1);
    tiers
}

/// 取 CSS 长度的像素值，仅用于排序权重（`rem` 按 16px 折算）
fn css_length_px(s: &str) -> u32 {
    let s = s.trim();
    let parsed = s
        .strip_suffix("px")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .or_else(|| {
            s.strip_suffix("rem")
                .and_then(|v| v.trim().parse::<f64>().ok())
                .map(|v| v * 16.0)
        });
    parsed.unwrap_or(640.0).round() as u32
}

/// 展开一个 at-rule 分组型工具类（`container` / `outline-hidden`）。
///
/// 规则数据全在 core 的 [`silex_tw_core::AT_RULE_UTILITIES`]，这里只负责把
/// "条件 + 声明"翻译成带 `AtRuleCondition` 修饰符的 `UtilityRule`。
pub fn at_rule_utility_to_rules(
    modifiers: &[SpannedModifier],
    meta: &'static silex_tw_core::AtRuleUtility,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let mut rules = Vec::new();

    for group in meta.groups {
        let mods = match group.media {
            Some(cond) => with_media(modifiers, cond, MEDIA_FEATURE_PRIORITY, span),
            None => modifiers.to_vec(),
        };
        for &(prop, val) in group.decls {
            rules.push(make_rule(
                mods.clone(),
                prop,
                to_utility_value(Cow::Borrowed(val)),
                span,
            )?);
        }
    }

    if let Some(prop) = meta.per_breakpoint {
        for (width, px) in container_tiers() {
            // 边界与 `min-[…]:` 同源：范围语法而不是 `min-width`，
            // 权重 `1000 + px` 让各档位按宽度升序层叠
            let mods = with_media(modifiers, &format!("(width >= {width})"), 1000 + px, span);
            rules.push(make_rule(
                mods,
                prop,
                to_utility_value(Cow::Owned(width)),
                span,
            )?);
        }
    }

    Ok(rules)
}

/// 把 core 的解析结果翻译成 `UtilityRule` 列表
pub fn rule_sets_to_rules(
    modifiers: &[SpannedModifier],
    sets: Vec<TwRuleSet>,
    span: Span,
) -> Result<Vec<UtilityRule>> {
    let mut rules = Vec::new();
    for set in sets {
        let mods = with_selector(modifiers, set.selector, span);
        for decl in set.decls {
            rules.push(make_rule(
                mods.clone(),
                decl.prop,
                to_utility_value(decl.value),
                span,
            )?);
        }
    }
    Ok(rules)
}
