/// Filter & Backdrop-Filter 规则解析
pub fn resolve_filter_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
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
        return Some(vec![(prop_name, "blur(8px)".to_string())]);
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
        return Some(vec![(prop_name, val.to_string())]);
    }

    // Brightness
    if let Some(rest) = target.strip_prefix("brightness-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(
                prop_name,
                format!("brightness({})", n as f64 / 100.0),
            )]);
        }
    }

    // Contrast
    if let Some(rest) = target.strip_prefix("contrast-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(prop_name, format!("contrast({})", n as f64 / 100.0))]);
        }
    }

    // Grayscale
    if target == "grayscale" {
        return Some(vec![(prop_name, "grayscale(100%)".to_string())]);
    }
    if target == "grayscale-0" {
        return Some(vec![(prop_name, "grayscale(0%)".to_string())]);
    }
    if let Some(rest) = target.strip_prefix("grayscale-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(prop_name, format!("grayscale({}%)", n))]);
        }
    }

    // Invert
    if target == "invert" {
        return Some(vec![(prop_name, "invert(100%)".to_string())]);
    }
    if target == "invert-0" {
        return Some(vec![(prop_name, "invert(0%)".to_string())]);
    }
    if let Some(rest) = target.strip_prefix("invert-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(prop_name, format!("invert({}%)", n))]);
        }
    }

    // Saturate
    if let Some(rest) = target.strip_prefix("saturate-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(prop_name, format!("saturate({})", n as f64 / 100.0))]);
        }
    }

    // Sepia
    if target == "sepia" {
        return Some(vec![(prop_name, "sepia(100%)".to_string())]);
    }
    if target == "sepia-0" {
        return Some(vec![(prop_name, "sepia(0%)".to_string())]);
    }
    if let Some(rest) = target.strip_prefix("sepia-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(prop_name, format!("sepia({}%)", n))]);
        }
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
    if let Some(rest) = target.strip_prefix("hue-rotate-") {
        if let Some(h) = parse_hue(rest) {
            return Some(vec![(prop_name, h)]);
        }
    }
    if let Some(rest) = target.strip_prefix("-hue-rotate-") {
        if let Some(h) = parse_hue(&format!("-{}", rest)) {
            return Some(vec![(prop_name, h)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-hue-rotate-") {
        if let Some(h) = parse_hue(&format!("-{}", rest)) {
            return Some(vec![("filter", h)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-backdrop-hue-rotate-") {
        if let Some(h) = parse_hue(&format!("-{}", rest)) {
            return Some(vec![("backdrop-filter", h)]);
        }
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
        return Some(vec![(prop_name, val.to_string())]);
    }

    // Opacity
    if is_backdrop {
        if let Some(rest) = target.strip_prefix("opacity-") {
            if let Ok(n) = rest.parse::<u32>() {
                return Some(vec![(
                    prop_name,
                    format!("opacity({})", (n as f64) / 100.0),
                )]);
            }
        }
    }

    None
}
