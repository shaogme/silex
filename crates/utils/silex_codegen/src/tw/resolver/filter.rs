use std::borrow::Cow;

/// Filter & Backdrop-Filter 规则解析
pub fn resolve_filter_rules(class_name: &str) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    // 静态 Blend Mode & Shadow 规则匹配
    let static_filter: Option<&'static [(&'static str, Cow<'static, str>)]> = match class_name {
        // Blend Modes
        "mix-blend-normal" => Some(cow![("mix-blend-mode", "normal")]),
        "mix-blend-multiply" => Some(cow![("mix-blend-mode", "multiply")]),
        "mix-blend-screen" => Some(cow![("mix-blend-mode", "screen")]),
        "mix-blend-overlay" => Some(cow![("mix-blend-mode", "overlay")]),
        "mix-blend-darken" => Some(cow![("mix-blend-mode", "darken")]),
        "mix-blend-lighten" => Some(cow![("mix-blend-mode", "lighten")]),
        "mix-blend-color-dodge" => Some(cow![("mix-blend-mode", "color-dodge")]),
        "mix-blend-color-burn" => Some(cow![("mix-blend-mode", "color-burn")]),
        "mix-blend-hard-light" => Some(cow![("mix-blend-mode", "hard-light")]),
        "mix-blend-soft-light" => Some(cow![("mix-blend-mode", "soft-light")]),
        "mix-blend-difference" => Some(cow![("mix-blend-mode", "difference")]),
        "mix-blend-exclusion" => Some(cow![("mix-blend-mode", "exclusion")]),
        "mix-blend-hue" => Some(cow![("mix-blend-mode", "hue")]),
        "mix-blend-saturation" => Some(cow![("mix-blend-mode", "saturation")]),
        "mix-blend-color" => Some(cow![("mix-blend-mode", "color")]),
        "mix-blend-luminosity" => Some(cow![("mix-blend-mode", "luminosity")]),
        "mix-blend-plus-lighter" => Some(cow![("mix-blend-mode", "plus-lighter")]),
        "mix-blend-plus-darker" => Some(cow![("mix-blend-mode", "plus-darker")]),

        "bg-blend-normal" => Some(cow![("background-blend-mode", "normal")]),
        "bg-blend-multiply" => Some(cow![("background-blend-mode", "multiply")]),
        "bg-blend-screen" => Some(cow![("background-blend-mode", "screen")]),
        "bg-blend-overlay" => Some(cow![("background-blend-mode", "overlay")]),
        "bg-blend-darken" => Some(cow![("background-blend-mode", "darken")]),
        "bg-blend-lighten" => Some(cow![("background-blend-mode", "lighten")]),
        "bg-blend-color-dodge" => Some(cow![("background-blend-mode", "color-dodge")]),
        "bg-blend-color-burn" => Some(cow![("background-blend-mode", "color-burn")]),
        "bg-blend-hard-light" => Some(cow![("background-blend-mode", "hard-light")]),
        "bg-blend-soft-light" => Some(cow![("background-blend-mode", "soft-light")]),
        "bg-blend-difference" => Some(cow![("background-blend-mode", "difference")]),
        "bg-blend-exclusion" => Some(cow![("background-blend-mode", "exclusion")]),
        "bg-blend-hue" => Some(cow![("background-blend-mode", "hue")]),
        "bg-blend-saturation" => Some(cow![("background-blend-mode", "saturation")]),
        "bg-blend-color" => Some(cow![("background-blend-mode", "color")]),
        "bg-blend-luminosity" => Some(cow![("background-blend-mode", "luminosity")]),

        // Shadow Initial
        "shadow-initial" => Some(cow![("box-shadow", "initial")]),
        "text-shadow-initial" => Some(cow![("text-shadow", "initial")]),
        "text-shadow-none" => Some(cow![("text-shadow", "none")]),

        _ => None,
    };
    if let Some(r) = static_filter {
        return Some(r.to_vec());
    }

    let (is_backdrop, target) = if let Some(rest) = class_name.strip_prefix("backdrop-") {
        (true, rest)
    } else {
        (false, class_name)
    };

    let prop_name = if is_backdrop {
        "backdrop-filter"
    } else {
        "filter"
    };

    // Blur
    if target == "blur" {
        return Some(cow!(vec![(prop_name, "blur(8px)")]));
    }
    if let Some(rest) = target.strip_prefix("blur-") {
        let val = match rest {
            "none" => "blur(0px)",
            "xs" | "2xs" | "3xs" => "blur(2px)",
            "sm" => "blur(4px)",
            "md" => "blur(8px)",
            "lg" => "blur(16px)",
            "xl" => "blur(24px)",
            "2xl" => "blur(40px)",
            "3xl" | "4xl" | "5xl" => "blur(64px)",
            _ => return None,
        };
        return Some(cow!(vec![(prop_name, val)]));
    }

    // Brightness
    if let Some(rest) = target.strip_prefix("brightness-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![(
            prop_name,
            format!("brightness({})", n as f64 / 100.0),
        )]));
    }

    // Contrast
    if let Some(rest) = target.strip_prefix("contrast-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![(
            prop_name,
            format!("contrast({})", n as f64 / 100.0),
        )]));
    }

    // Grayscale
    if target == "grayscale" {
        return Some(cow!(vec![(prop_name, "grayscale(100%)")]));
    }
    if target == "grayscale-0" {
        return Some(cow!(vec![(prop_name, "grayscale(0%)")]));
    }
    if let Some(rest) = target.strip_prefix("grayscale-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![(prop_name, format!("grayscale({}%)", n))]));
    }

    // Invert
    if target == "invert" {
        return Some(cow!(vec![(prop_name, "invert(100%)")]));
    }
    if target == "invert-0" {
        return Some(cow!(vec![(prop_name, "invert(0%)")]));
    }
    if let Some(rest) = target.strip_prefix("invert-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![(prop_name, format!("invert({}%)", n))]));
    }

    // Saturate
    if let Some(rest) = target.strip_prefix("saturate-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![(
            prop_name,
            format!("saturate({})", n as f64 / 100.0),
        )]));
    }

    // Sepia
    if target == "sepia" {
        return Some(cow!(vec![(prop_name, "sepia(100%)")]));
    }
    if target == "sepia-0" {
        return Some(cow!(vec![(prop_name, "sepia(0%)")]));
    }
    if let Some(rest) = target.strip_prefix("sepia-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![(prop_name, format!("sepia({}%)", n))]));
    }

    // Hue-Rotate (supports positive and negative e.g. -hue-rotate-90, -backdrop-hue-rotate-90)
    let parse_hue = |s: &str| -> Option<String> {
        let (neg, rest) = if let Some(r) = s.strip_prefix('-') {
            (true, r)
        } else {
            (false, s)
        };
        if let Ok(n) = rest.parse::<u32>() {
            Some(if neg {
                format!("hue-rotate(-{}deg)", n)
            } else {
                format!("hue-rotate({}deg)", n)
            })
        } else {
            None
        }
    };
    if let Some(rest) = target.strip_prefix("hue-rotate-")
        && let Some(h) = parse_hue(rest)
    {
        return Some(cow!(vec![(prop_name, h)]));
    }
    if let Some(rest) = target.strip_prefix("-hue-rotate-")
        && let Some(h) = parse_hue(&format!("-{}", rest))
    {
        return Some(cow!(vec![(prop_name, h)]));
    }
    if let Some(rest) = class_name.strip_prefix("-hue-rotate-")
        && let Some(h) = parse_hue(&format!("-{}", rest))
    {
        return Some(cow!(vec![("filter", h)]));
    }
    if let Some(rest) = class_name.strip_prefix("-backdrop-hue-rotate-")
        && let Some(h) = parse_hue(&format!("-{}", rest))
    {
        return Some(cow!(vec![("backdrop-filter", h)]));
    }

    // Drop Shadow
    if let Some(rest) = target.strip_prefix("drop-shadow") {
        let val = match rest {
            "-sm" => "drop-shadow(0 1px 1px rgba(0, 0, 0, 0.05))",
            "" => {
                "drop-shadow(0 1px 2px rgba(0, 0, 0, 0.1)) drop-shadow(0 1px 1px rgba(0, 0, 0, 0.06))"
            }
            "-md" => {
                "drop-shadow(0 4px 3px rgba(0, 0, 0, 0.07)) drop-shadow(0 2px 2px rgba(0, 0, 0, 0.06))"
            }
            "-lg" => {
                "drop-shadow(0 10px 8px rgba(0, 0, 0, 0.04)) drop-shadow(0 4px 3px rgba(0, 0, 0, 0.1))"
            }
            "-xl" => {
                "drop-shadow(0 20px 13px rgba(0, 0, 0, 0.03)) drop-shadow(0 8px 5px rgba(0, 0, 0, 0.08))"
            }
            "-2xl" => "drop-shadow(0 25px 25px rgba(0, 0, 0, 0.15))",
            "-none" => "drop-shadow(0 0 #0000)",
            _ => return None,
        };
        return Some(cow!(vec![(prop_name, val)]));
    }

    // Opacity
    if is_backdrop
        && let Some(rest) = target.strip_prefix("opacity-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![(
            prop_name,
            format!("opacity({})", (n as f64) / 100.0),
        )]));
    }

    None
}
