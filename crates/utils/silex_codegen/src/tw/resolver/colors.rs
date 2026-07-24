use crate::tw::ColorShadeInfo;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// 颜色规则表解析
pub fn resolve_color_rules<'a>(
    class_name: &str,
    palette: &'a BTreeMap<String, Vec<ColorShadeInfo>>,
) -> Option<Vec<(&'static str, Cow<'a, str>)>> {
    let color_prefixes = &[
        ("scrollbar-thumb-", "scrollbar-color"),
        ("scrollbar-track-", "scrollbar-color"),
        ("inset-shadow-", "--tw-inset-shadow-color"),
        ("drop-shadow-", "--tw-drop-shadow-color"),
        ("text-shadow-", "--tw-text-shadow-color"),
        ("ring-offset-", "--tw-ring-offset-color"),
        ("placeholder-", "color"),
        ("decoration-", "text-decoration-color"),
        ("inset-ring-", "--tw-inset-ring-color"),
        ("border-bs-", "border-block-start-color"),
        ("border-be-", "border-block-end-color"),
        ("border-b-", "border-bottom-color"),
        ("border-t-", "border-top-color"),
        ("border-l-", "border-left-color"),
        ("border-r-", "border-right-color"),
        ("border-s-", "border-inline-start-color"),
        ("border-e-", "border-inline-end-color"),
        ("border-x-", "border-inline-color"),
        ("border-y-", "border-block-color"),
        ("outline-", "outline-color"),
        ("border-", "border-color"),
        ("accent-", "accent-color"),
        ("divide-", "border-color"),
        ("stroke-", "stroke"),
        ("shadow-", "--tw-shadow-color"),
        ("caret-", "caret-color"),
        ("text-", "color"),
        ("fill-", "fill"),
        ("from-", "--tw-gradient-from"),
        ("ring-", "outline-color"),
        ("via-", "--tw-gradient-via"),
        ("bg-", "background-color"),
        ("to-", "--tw-gradient-to"),
    ];

    for &(prefix, prop) in color_prefixes {
        if let Some(color_name) = class_name.strip_prefix(prefix)
            && let Some(hex) = resolve_color_hex(color_name, palette)
        {
            return Some(cow!(vec[(prop, hex)]));
        }
    }

    if let Some(rest) = class_name.strip_prefix("mask-") {
        if let Some(idx) = rest.rfind("-from-") {
            let val_name = &rest[idx + 6..];
            if let Some(hex) = resolve_color_hex(val_name, palette) {
                return Some(cow!(vec![("--tw-mask-from", hex)]));
            }
            if let Some(len) = super::dynamic::resolve_length_val(val_name) {
                return Some(cow!(vec![("--tw-mask-from-position", len)]));
            }
        }
        if let Some(idx) = rest.rfind("-to-") {
            let val_name = &rest[idx + 4..];
            if let Some(hex) = resolve_color_hex(val_name, palette) {
                return Some(cow!(vec![("--tw-mask-to", hex)]));
            }
            if let Some(len) = super::dynamic::resolve_length_val(val_name) {
                return Some(cow!(vec![("--tw-mask-to-position", len)]));
            }
        }
    }

    None
}

/// Tailwind CSS 标准颜色面板 (50..950 + 基础色彩)
pub fn resolve_color_hex<'a>(
    color_name: &str,
    palette: &'a BTreeMap<String, Vec<ColorShadeInfo>>,
) -> Option<&'a str> {
    match color_name {
        "transparent" => Some("transparent"),
        "current" => Some("currentColor"),
        "black" => Some("#000000"),
        "white" => Some("#ffffff"),
        "inherit" => Some("inherit"),
        _ => {
            let (family, shade) = color_name.rsplit_once('-')?;
            let shades = palette.get(family)?;
            let info = shades.iter().find(|s| s.shade == shade)?;
            Some(info.hex.as_str())
        }
    }
}
