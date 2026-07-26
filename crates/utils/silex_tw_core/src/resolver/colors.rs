//! `mask-*-from-` / `mask-*-to-` 的颜色与位置解析。
//!
//! 通用的颜色前缀映射已经收敛到 [`crate::prefix::COLOR_PREFIX_RULES`] 与
//! [`crate::color::resolve_color_utility`]，本模块只保留 mask 这种
//! "同一段后缀既可能是颜色也可能是长度"的特例。

use std::borrow::Cow;

use crate::{color::parse_color_value, context::TwContext};

/// 解析 `mask-<方向>-from-<值>` / `mask-<方向>-to-<值>`，值可为颜色或长度
pub fn resolve_mask_color_rules(
    class_name: &str,
    ctx: &dyn TwContext,
) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    let rest = class_name.strip_prefix("mask-")?;

    for (marker, color_prop, position_prop) in [
        ("-from-", "--tw-mask-from", "--tw-mask-from-position"),
        ("-to-", "--tw-mask-to", "--tw-mask-to-position"),
    ] {
        let Some(idx) = rest.rfind(marker) else {
            continue;
        };
        let val_name = &rest[idx + marker.len()..];

        if let Some(color) = parse_color_value(ctx, val_name) {
            return Some(vec![(color_prop, color)]);
        }
        if let Some(len) = super::dynamic::resolve_length_val(val_name) {
            return Some(vec![(position_prop, Cow::Owned(len))]);
        }
    }

    None
}
