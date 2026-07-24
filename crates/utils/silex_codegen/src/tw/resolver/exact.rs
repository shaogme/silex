use std::borrow::Cow;

mod interactivity;
mod layout;
mod mask;
mod tables_lists_svg;

use interactivity::resolve_interactivity_rules;
use layout::resolve_layout_rules;
use mask::resolve_mask_rules;
use tables_lists_svg::resolve_tables_lists_svg_rules;

/// 精准静态匹配表
pub fn resolve_exact_match(
    class_name: &str,
) -> Option<&'static [(&'static str, Cow<'static, str>)]> {
    if let Some(rules) = resolve_layout_rules(class_name) {
        return Some(rules);
    }
    if let Some(rules) = resolve_interactivity_rules(class_name) {
        return Some(rules);
    }
    if let Some(rules) = resolve_mask_rules(class_name) {
        return Some(rules);
    }
    resolve_tables_lists_svg_rules(class_name)
}
