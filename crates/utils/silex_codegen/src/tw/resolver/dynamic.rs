/// 解析数值与长度（rem, %, px, auto等），并正确处理负号
pub fn resolve_length_val(val_str: &str) -> Option<String> {
    let (is_negative, s) = if let Some(stripped) = val_str.strip_prefix('-') {
        (true, stripped)
    } else {
        (false, val_str)
    };

    let len = match s {
        "0" => Some("0px".to_string()),
        "px" => Some("1px".to_string()),
        "0.5" => Some("0.125rem".to_string()),
        "1" => Some("0.25rem".to_string()),
        "1.5" => Some("0.375rem".to_string()),
        "2" => Some("0.5rem".to_string()),
        "2.5" => Some("0.625rem".to_string()),
        "3" => Some("0.75rem".to_string()),
        "3.5" => Some("0.875rem".to_string()),
        "4" => Some("1rem".to_string()),
        "5" => Some("1.25rem".to_string()),
        "6" => Some("1.5rem".to_string()),
        "7" => Some("1.75rem".to_string()),
        "8" => Some("2rem".to_string()),
        "9" => Some("2.25rem".to_string()),
        "10" => Some("2.5rem".to_string()),
        "11" => Some("2.75rem".to_string()),
        "12" => Some("3rem".to_string()),
        "14" => Some("3.5rem".to_string()),
        "16" => Some("4rem".to_string()),
        "20" => Some("5rem".to_string()),
        "24" => Some("6rem".to_string()),
        "28" => Some("7rem".to_string()),
        "32" => Some("8rem".to_string()),
        "36" => Some("9rem".to_string()),
        "40" => Some("10rem".to_string()),
        "44" => Some("11rem".to_string()),
        "48" => Some("12rem".to_string()),
        "52" => Some("13rem".to_string()),
        "56" => Some("14rem".to_string()),
        "60" => Some("15rem".to_string()),
        "64" => Some("16rem".to_string()),
        "72" => Some("18rem".to_string()),
        "80" => Some("20rem".to_string()),
        "96" => Some("24rem".to_string()),
        "1/2" => Some("50%".to_string()),
        "1/3" => Some("33.333333%".to_string()),
        "2/3" => Some("66.666667%".to_string()),
        "1/4" => Some("25%".to_string()),
        "3/4" => Some("75%".to_string()),
        "1/5" => Some("20%".to_string()),
        "2/5" => Some("40%".to_string()),
        "3/5" => Some("60%".to_string()),
        "4/5" => Some("80%".to_string()),
        "1/6" => Some("16.666667%".to_string()),
        "2/6" => Some("33.333333%".to_string()),
        "3/6" => Some("50%".to_string()),
        "4/6" => Some("66.666667%".to_string()),
        "5/6" => Some("83.333333%".to_string()),
        "2/4" => Some("50%".to_string()),
        "1/12" => Some("8.333333%".to_string()),
        "2/12" => Some("16.666667%".to_string()),
        "3/12" => Some("25%".to_string()),
        "4/12" => Some("33.333333%".to_string()),
        "5/12" => Some("41.666667%".to_string()),
        "6/12" => Some("50%".to_string()),
        "7/12" => Some("58.333333%".to_string()),
        "8/12" => Some("66.666667%".to_string()),
        "9/12" => Some("75%".to_string()),
        "10/12" => Some("83.333333%".to_string()),
        "11/12" => Some("91.666667%".to_string()),
        "full" => Some("100%".to_string()),
        "screen" => Some("100vh".to_string()),
        "dvh" => Some("100dvh".to_string()),
        "lvh" => Some("100lvh".to_string()),
        "svh" => Some("100svh".to_string()),
        "dvw" => Some("100dvw".to_string()),
        "lvw" => Some("100lvw".to_string()),
        "svw" => Some("100svw".to_string()),
        "vw" => Some("100vw".to_string()),
        "vh" => Some("100vh".to_string()),
        "lh" => Some("1lh".to_string()),
        "auto" => Some("auto".to_string()),
        "min" => Some("min-content".to_string()),
        "max" => Some("max-content".to_string()),
        "fit" => Some("fit-content".to_string()),
        "3xs" => Some("16rem".to_string()),
        "2xs" => Some("18rem".to_string()),
        "xs" => Some("20rem".to_string()),
        "sm" => Some("24rem".to_string()),
        "md" => Some("28rem".to_string()),
        "lg" => Some("32rem".to_string()),
        "xl" => Some("36rem".to_string()),
        "2xl" => Some("42rem".to_string()),
        "3xl" => Some("48rem".to_string()),
        "4xl" => Some("56rem".to_string()),
        "5xl" => Some("64rem".to_string()),
        "6xl" => Some("72rem".to_string()),
        "7xl" => Some("80rem".to_string()),
        "8xl" => Some("96rem".to_string()),
        "9xl" => Some("128rem".to_string()),
        "none" => Some("none".to_string()),
        _ => {
            if let Ok(n) = s.parse::<f64>() {
                Some(format!("{}rem", n * 0.25))
            } else {
                None
            }
        }
    }?;

    if is_negative && len != "0px" && len != "auto" {
        Some(format!("-{}", len))
    } else {
        Some(len)
    }
}

/// 动态内边距、外边距、定位、定位逻辑属性、尺寸、Z-index、透明度
pub fn resolve_dynamic_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    let (prefix, rest) = if let Some(r) = class_name.strip_prefix('-') {
        ("-", r)
    } else {
        ("", class_name)
    };

    let prefixes_map: &[(&str, &[&'static str])] = &[
        ("scroll-mbs-", &["scroll-margin-block-start"]),
        ("scroll-mbe-", &["scroll-margin-block-end"]),
        ("scroll-pbs-", &["scroll-padding-block-start"]),
        ("scroll-pbe-", &["scroll-padding-block-end"]),
        ("scroll-mx-", &["scroll-margin-left", "scroll-margin-right"]),
        ("scroll-my-", &["scroll-margin-top", "scroll-margin-bottom"]),
        ("scroll-mt-", &["scroll-margin-top"]),
        ("scroll-mr-", &["scroll-margin-right"]),
        ("scroll-mb-", &["scroll-margin-bottom"]),
        ("scroll-ml-", &["scroll-margin-left"]),
        ("scroll-ms-", &["scroll-margin-inline-start"]),
        ("scroll-me-", &["scroll-margin-inline-end"]),
        ("scroll-px-", &["scroll-padding-left", "scroll-padding-right"]),
        ("scroll-py-", &["scroll-padding-top", "scroll-padding-bottom"]),
        ("scroll-pt-", &["scroll-padding-top"]),
        ("scroll-pr-", &["scroll-padding-right"]),
        ("scroll-pb-", &["scroll-padding-bottom"]),
        ("scroll-pl-", &["scroll-padding-left"]),
        ("scroll-ps-", &["scroll-padding-inline-start"]),
        ("scroll-pe-", &["scroll-padding-inline-end"]),
        ("min-inline-", &["min-inline-size"]),
        ("max-inline-", &["max-inline-size"]),
        ("min-block-", &["min-block-size"]),
        ("max-block-", &["max-block-size"]),
        ("scroll-m-", &["scroll-margin"]),
        ("scroll-p-", &["scroll-padding"]),
        ("inset-bs-", &["inset-block-start"]),
        ("inset-be-", &["inset-block-end"]),
        ("inset-x-", &["left", "right"]),
        ("inset-y-", &["top", "bottom"]),
        ("inset-s-", &["inset-inline-start"]),
        ("inset-e-", &["inset-inline-end"]),
        ("space-x-", &["margin-left"]),
        ("space-y-", &["margin-top"]),
        ("min-w-", &["min-width"]),
        ("max-w-", &["max-width"]),
        ("min-h-", &["min-height"]),
        ("max-h-", &["max-height"]),
        ("inline-", &["inline-size"]),
        ("bottom-", &["bottom"]),
        ("right-", &["right"]),
        ("inset-", &["top", "right", "bottom", "left"]),
        ("basis-", &["flex-basis"]),
        ("block-", &["block-size"]),
        ("gap-x-", &["column-gap"]),
        ("gap-y-", &["row-gap"]),
        ("start-", &["inset-inline-start"]),
        ("left-", &["left"]),
        ("size-", &["width", "height"]),
        ("pbs-", &["padding-block-start"]),
        ("pbe-", &["padding-block-end"]),
        ("mbs-", &["margin-block-start"]),
        ("mbe-", &["margin-block-end"]),
        ("gap-", &["gap"]),
        ("top-", &["top"]),
        ("end-", &["inset-inline-end"]),
        ("px-", &["padding-left", "padding-right"]),
        ("py-", &["padding-top", "padding-bottom"]),
        ("pt-", &["padding-top"]),
        ("pr-", &["padding-right"]),
        ("pb-", &["padding-bottom"]),
        ("pl-", &["padding-left"]),
        ("ps-", &["padding-inline-start"]),
        ("pe-", &["padding-inline-end"]),
        ("mx-", &["margin-left", "margin-right"]),
        ("my-", &["margin-top", "margin-bottom"]),
        ("mt-", &["margin-top"]),
        ("mr-", &["margin-right"]),
        ("mb-", &["margin-bottom"]),
        ("ml-", &["margin-left"]),
        ("ms-", &["margin-inline-start"]),
        ("me-", &["margin-inline-end"]),
        ("p-", &["padding"]),
        ("m-", &["margin"]),
        ("w-", &["width"]),
        ("h-", &["height"]),
    ];


    for (p, props) in prefixes_map {
        if rest.starts_with(p) {
            let val_part = &rest[p.len()..];
            let full_val_str = if prefix == "-" {
                format!("-{}", val_part)
            } else {
                val_part.to_string()
            };
            if let Some(val) = resolve_length_val(&full_val_str) {
                let rules = props.iter().map(|&pr| (pr, val.clone())).collect();
                return Some(rules);
            }
        }
    }

    // Standalone defaults (bottom, top, left, right, inset, m, p, w, h, gap, etc.)
    let standalone = match class_name {
        "bottom" => Some(vec![("bottom", "0px")]),
        "top" => Some(vec![("top", "0px")]),
        "left" => Some(vec![("left", "0px")]),
        "right" => Some(vec![("right", "0px")]),
        "inset" => Some(vec![("top", "0px"), ("right", "0px"), ("bottom", "0px"), ("left", "0px")]),
        "m" => Some(vec![("margin", "0px")]),
        "p" => Some(vec![("padding", "0px")]),
        "mt" => Some(vec![("margin-top", "0px")]),
        "mr" => Some(vec![("margin-right", "0px")]),
        "mb" => Some(vec![("margin-bottom", "0px")]),
        "ml" => Some(vec![("margin-left", "0px")]),
        "mx" => Some(vec![("margin-left", "0px"), ("margin-right", "0px")]),
        "my" => Some(vec![("margin-top", "0px"), ("margin-bottom", "0px")]),
        "pt" => Some(vec![("padding-top", "0px")]),
        "pr" => Some(vec![("padding-right", "0px")]),
        "pb" => Some(vec![("padding-bottom", "0px")]),
        "pl" => Some(vec![("padding-left", "0px")]),
        "px" => Some(vec![("padding-left", "0px"), ("padding-right", "0px")]),
        "py" => Some(vec![("padding-top", "0px"), ("padding-bottom", "0px")]),
        "w" => Some(vec![("width", "100%")]),
        "h" => Some(vec![("height", "100%")]),
        "min-w" => Some(vec![("min-width", "0px")]),
        "max-w" => Some(vec![("max-width", "none")]),
        "min-h" => Some(vec![("min-height", "0px")]),
        "max-h" => Some(vec![("max-height", "none")]),
        "gap" => Some(vec![("gap", "0px")]),
        "columns" => Some(vec![("column-count", "auto")]),
        _ => None,
    };
    if let Some(rules) = standalone {
        return Some(rules.into_iter().map(|(k, v)| (k, v.to_string())).collect());
    }

    // Space Reverse
    if class_name == "space-x-reverse" {
        return Some(vec![("--tw-space-x-reverse", "1".to_string())]);
    }
    if class_name == "space-y-reverse" {
        return Some(vec![("--tw-space-y-reverse", "1".to_string())]);
    }

    // Max Size None
    if class_name == "max-w-none" {
        return Some(vec![("max-width", "none".to_string())]);
    }
    if class_name == "max-h-none" {
        return Some(vec![("max-height", "none".to_string())]);
    }
    if class_name == "max-block-none" {
        return Some(vec![("max-block-size", "none".to_string())]);
    }
    if class_name == "max-inline-none" {
        return Some(vec![("max-inline-size", "none".to_string())]);
    }

    // Gradient Stop Positions (from-0%, via-50%, to-100%, etc.)
    let parse_percent = |s: &str| -> Option<String> {
        let p_str = s.strip_suffix('%')?;
        if p_str.parse::<u32>().is_ok() {
            Some(format!("{}%", p_str))
        } else {
            None
        }
    };

    if let Some(rest) = class_name.strip_prefix("from-") {
        if let Some(pct) = parse_percent(rest) {
            return Some(vec![("--tw-gradient-from-position", pct)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("via-") {
        if let Some(pct) = parse_percent(rest) {
            return Some(vec![("--tw-gradient-via-position", pct)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("to-") {
        if let Some(pct) = parse_percent(rest) {
            return Some(vec![("--tw-gradient-to-position", pct)]);
        }
    }

    // Mask Stop Positions & Directions
    if let Some(rest) = class_name.strip_prefix("mask-") {
        if rest.starts_with("conic-from-") || rest.starts_with("radial-from-") || rest.starts_with("linear-from-") || rest.starts_with("b-from-") || rest.starts_with("t-from-") || rest.starts_with("l-from-") || rest.starts_with("r-from-") || rest.starts_with("x-from-") || rest.starts_with("y-from-") {
            if let Some(idx) = rest.rfind("-from-") {
                let pct_str = &rest[idx + 6..];
                if let Some(pct) = parse_percent(pct_str) {
                    return Some(vec![("--tw-mask-from-position", pct)]);
                }
            }
        }
        if rest.starts_with("conic-to-") || rest.starts_with("radial-to-") || rest.starts_with("linear-to-") || rest.starts_with("b-to-") || rest.starts_with("t-to-") || rest.starts_with("l-to-") || rest.starts_with("r-to-") || rest.starts_with("x-to-") || rest.starts_with("y-to-") {
            if let Some(idx) = rest.rfind("-to-") {
                let pct_str = &rest[idx + 4..];
                if let Some(pct) = parse_percent(pct_str) {
                    return Some(vec![("--tw-mask-to-position", pct)]);
                }
            }
        }
    }

    // Z-index
    if let Some(val_str) = class_name.strip_prefix("z-") {
        if val_str == "auto" || val_str.parse::<i32>().is_ok() {
            return Some(vec![("z-index", val_str.to_string())]);
        }
    }
    if let Some(val_str) = class_name.strip_prefix("-z-") {
        if let Ok(num) = val_str.parse::<i32>() {
            return Some(vec![("z-index", format!("-{}", num))]);
        }
    }

    // Opacity
    if let Some(val_str) = class_name.strip_prefix("opacity-") {
        if let Ok(num) = val_str.parse::<u32>() {
            let op = (num as f64) / 100.0;
            return Some(vec![("opacity", op.to_string())]);
        }
    }

    // Linear / Conic Gradients (bg-linear-*, bg-conic-*, mask-linear-*, mask-conic-*)
    let (is_neg, rest_name) = if let Some(r) = class_name.strip_prefix('-') {
        (true, r)
    } else {
        (false, class_name)
    };

    if let Some(deg_str) = rest_name.strip_prefix("bg-linear-") {
        if let Ok(deg) = deg_str.parse::<u32>() {
            let angle = if is_neg { format!("-{}deg", deg) } else { format!("{}deg", deg) };
            return Some(vec![("background-image", format!("linear-gradient({}, var(--tw-gradient-stops))", angle))]);
        }
    }
    if let Some(deg_str) = rest_name.strip_prefix("bg-conic-") {
        if let Ok(deg) = deg_str.parse::<u32>() {
            let angle = if is_neg { format!("-{}deg", deg) } else { format!("{}deg", deg) };
            return Some(vec![("background-image", format!("conic-gradient(from {}, var(--tw-gradient-stops))", angle))]);
        }
    }
    if let Some(deg_str) = rest_name.strip_prefix("mask-linear-") {
        if let Ok(deg) = deg_str.parse::<u32>() {
            let angle = if is_neg { format!("-{}deg", deg) } else { format!("{}deg", deg) };
            return Some(vec![("mask-image", format!("linear-gradient({}, var(--tw-mask-stops))", angle))]);
        }
    }
    if let Some(deg_str) = rest_name.strip_prefix("mask-conic-") {
        if let Ok(deg) = deg_str.parse::<u32>() {
            let angle = if is_neg { format!("-{}deg", deg) } else { format!("{}deg", deg) };
            return Some(vec![("mask-image", format!("conic-gradient(from {}, var(--tw-mask-stops))", angle))]);
        }
    }

    None
}
