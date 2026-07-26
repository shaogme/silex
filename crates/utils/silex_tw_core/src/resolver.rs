use std::borrow::Cow;

pub mod between;
pub mod colors;
pub mod dynamic;
pub mod exact;
pub mod filter;
pub mod flex_grid;
pub mod transforms;
pub mod typography_border;

use between::resolve_between_rules;
use colors::resolve_mask_color_rules;
use dynamic::resolve_dynamic_rules;
use exact::resolve_exact_match;
use filter::resolve_filter_rules;
use flex_grid::resolve_flex_grid_rules;
use transforms::resolve_transform_transition_rules;
use typography_border::resolve_typography_border_effect_rules;

use crate::{
    color::resolve_color_utility,
    context::TwContext,
    value::{TwDecl, TwRuleSet},
};

/// 把 `(属性, 值)` 序列包成一组作用于元素自身的声明
fn plain<'a>(pairs: impl IntoIterator<Item = (&'static str, Cow<'a, str>)>) -> Vec<TwRuleSet> {
    vec![TwRuleSet::plain(
        pairs
            .into_iter()
            .map(|(prop, value)| TwDecl::new(prop, value.into_owned()))
            .collect(),
    )]
}

/// 解析一个 Tailwind 类名。
///
/// 这是**两侧共用**的入口：`silex_codegen` 用它把 `classes.json` 批量求值成静态表，
/// `silex_macros` 在静态表未命中时用它做兜底。规则只有这一份。
pub fn resolve_class(class_name: &str, ctx: &dyn TwContext) -> Option<Vec<TwRuleSet>> {
    // 1. 静态精准匹配 (Layout, Interactivity, Mask, Tables/Lists/SVG)
    if let Some(rules) = resolve_exact_match(class_name) {
        return Some(plain(rules.iter().cloned()));
    }

    // 2. 颜色属性匹配 (bg-*, text-*, border-*, ring-*, divide-*, placeholder-*, 渐变色标 …)
    if let Some(sets) = resolve_color_utility(ctx, class_name) {
        return Some(sets);
    }

    // 3. mask 的颜色/位置双关后缀
    if let Some(rules) = resolve_mask_color_rules(class_name, ctx) {
        return Some(plain(rules));
    }

    // 4. divide-* / space-* 的尺寸与线型：声明落在相邻子元素之间
    if let Some(sets) = resolve_between_rules(class_name) {
        return Some(sets);
    }

    // 5. 边框、圆角、阴影、字体尺寸/字重/行高/字距等匹配
    if let Some(rules) = resolve_typography_border_effect_rules(class_name) {
        return Some(plain(rules));
    }

    // 6. Flexbox & Grid 匹配 (grid-cols, flex-row, justify-*, items-*, etc.)
    if let Some(rules) = resolve_flex_grid_rules(class_name) {
        return Some(plain(rules));
    }

    // 7. Transform & Transition & Animation 匹配 (scale, rotate, duration, transform, etc.)
    if let Some(rules) = resolve_transform_transition_rules(class_name) {
        return Some(plain(rules));
    }

    // 8. Filter & Backdrop Filter 匹配 (blur, brightness, grayscale, mix-blend-*, etc.)
    if let Some(rules) = resolve_filter_rules(class_name) {
        return Some(plain(rules));
    }

    // 9. 动态长度与位置匹配 (Spacing, Sizing, Offset, Z-index, Opacity)
    resolve_dynamic_rules(class_name).map(plain)
}
