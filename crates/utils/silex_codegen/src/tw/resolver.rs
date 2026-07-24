use crate::tw::ColorShadeInfo;
use std::borrow::Cow;
use std::collections::BTreeMap;

pub mod colors;
pub mod dynamic;
pub mod exact;
pub mod filter;
pub mod flex_grid;
pub mod transforms;
pub mod typography_border;

use colors::resolve_color_rules;
use dynamic::resolve_dynamic_rules;
use exact::resolve_exact_match;
use filter::resolve_filter_rules;
use flex_grid::resolve_flex_grid_rules;
use transforms::resolve_transform_transition_rules;
use typography_border::resolve_typography_border_effect_rules;

/// 解析 Tailwind 类名对应的 CSS 规则对 `(property, value)`
pub fn resolve_css_rules<'a>(
    class_name: &str,
    palette: &'a BTreeMap<String, Vec<ColorShadeInfo>>,
) -> Option<Vec<(&'static str, Cow<'a, str>)>> {
    // 1. 静态精准匹配 (Layout, Interactivity, Mask, Tables/Lists/SVG)
    if let Some(rules) = resolve_exact_match(class_name) {
        return Some(rules.to_vec());
    }

    // 2. 色彩属性匹配 (bg-*, text-*, border-*, ring-*, fill-*, stroke-*, etc.)
    if let Some(rules) = resolve_color_rules(class_name, palette) {
        return Some(rules);
    }

    // 3. 边框、圆角、阴影、字体尺寸/字重/行高/字距等匹配
    if let Some(rules) = resolve_typography_border_effect_rules(class_name) {
        return Some(rules);
    }

    // 4. Flexbox & Grid 匹配 (grid-cols, flex-row, justify-*, items-*, etc.)
    if let Some(rules) = resolve_flex_grid_rules(class_name) {
        return Some(rules);
    }

    // 5. Transform & Transition & Animation 匹配 (scale, rotate, duration, transform, etc.)
    if let Some(rules) = resolve_transform_transition_rules(class_name) {
        return Some(rules);
    }

    // 6. Filter & Backdrop Filter 匹配 (blur, brightness, grayscale, mix-blend-*, etc.)
    if let Some(rules) = resolve_filter_rules(class_name) {
        return Some(rules);
    }

    // 7. 动态长度与位置匹配 (Spacing, Sizing, Offset, Z-index, Opacity)
    resolve_dynamic_rules(class_name)
}
