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
