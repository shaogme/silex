use super::dynamic::resolve_length_val;

/// Transform & Transition & Animation & Gradients
pub fn resolve_transform_transition_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    // Animations
    match class_name {
        "animate" | "animate-spin" => return Some(vec![("animation", "spin 1s linear infinite".to_string()), ("will-change", "transform".to_string())]),
        "animate-ping" => return Some(vec![("animation", "ping 1s cubic-bezier(0, 0, 0.2, 1) infinite".to_string()), ("will-change", "transform, opacity".to_string())]),
        "animate-pulse" => return Some(vec![("animation", "pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite".to_string()), ("will-change", "opacity".to_string())]),
        "animate-bounce" => return Some(vec![("animation", "bounce 1s infinite".to_string()), ("will-change", "transform".to_string())]),
        "animate-none" => return Some(vec![("animation", "none".to_string())]),
        _ => {}
    }

    // Transition duration / delay / ease
    if let Some(rest) = class_name.strip_prefix("duration-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("transition-duration", format!("{}ms", n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("delay-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("transition-delay", format!("{}ms", n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("ease-") {
        let timing = match rest {
            "linear" => Some("linear"),
            "in" => Some("cubic-bezier(0.4, 0, 1, 1)"),
            "out" => Some("cubic-bezier(0, 0, 0.2, 1)"),
            "in-out" => Some("cubic-bezier(0.4, 0, 0.2, 1)"),
            "initial" => Some("initial"),
            _ => None,
        };
        if let Some(t) = timing {
            return Some(vec![("transition-timing-function", t.to_string())]);
        }
    }

    // Gradients
    if class_name == "bg-none" {
        return Some(vec![("background-image", "none".to_string())]);
    }
    if let Some(rest) = class_name.strip_prefix("bg-gradient-to-") {
        let dir = match rest {
            "r" => Some("to right"),
            "l" => Some("to left"),
            "t" => Some("to top"),
            "b" => Some("to bottom"),
            "tr" => Some("to top right"),
            "br" => Some("to bottom right"),
            "tl" => Some("to top left"),
            "bl" => Some("to bottom left"),
            _ => None,
        };
        if let Some(d) = dir {
            return Some(vec![("background-image", format!("linear-gradient({}, var(--tw-gradient-stops))", d))]);
        }
    }
    // Scale
    let scale_ratio = |s: &str| -> Option<String> {
        let (neg, val) = if let Some(stripped) = s.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, s)
        };
        if let Ok(n) = val.parse::<f64>() {
            let r = n / 100.0;
            Some(if neg { format!("-{}", r) } else { format!("{}", r) })
        } else {
            None
        }
    };

    if class_name == "scale-none" {
        return Some(vec![("scale", "none".to_string())]);
    }
    if class_name == "scale-3d" {
        return Some(vec![("transform-style", "preserve-3d".to_string())]);
    }
    if let Some(rest) = class_name.strip_prefix("scale-x-") {
        if let Some(r) = scale_ratio(rest) {
            return Some(vec![("scale", format!("{} 1", r))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-scale-x-") {
        if let Some(r) = scale_ratio(&format!("-{}", rest)) {
            return Some(vec![("scale", format!("{} 1", r))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("scale-y-") {
        if let Some(r) = scale_ratio(rest) {
            return Some(vec![("scale", format!("1 {}", r))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-scale-y-") {
        if let Some(r) = scale_ratio(&format!("-{}", rest)) {
            return Some(vec![("scale", format!("1 {}", r))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("scale-z-") {
        if let Some(r) = scale_ratio(rest) {
            return Some(vec![("scale", format!("1 1 {}", r))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-scale-z-") {
        if let Some(r) = scale_ratio(&format!("-{}", rest)) {
            return Some(vec![("scale", format!("1 1 {}", r))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("scale-") {
        if let Some(r) = scale_ratio(rest) {
            return Some(vec![("scale", r)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-scale-") {
        if let Some(r) = scale_ratio(&format!("-{}", rest)) {
            return Some(vec![("scale", r)]);
        }
    }

    // Rotate
    let deg_val = |s: &str| -> Option<String> {
        let (neg, val) = if let Some(stripped) = s.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, s)
        };
        if val.parse::<u32>().is_ok() {
            Some(if neg { format!("-{}deg", val) } else { format!("{}deg", val) })
        } else {
            None
        }
    };
    if class_name == "rotate-none" {
        return Some(vec![("rotate", "none".to_string())]);
    }
    if let Some(rest) = class_name.strip_prefix("rotate-x-") {
        if let Some(deg) = deg_val(rest) {
            return Some(vec![("transform", format!("rotateX({})", deg))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-x-") {
        if let Some(deg) = deg_val(&format!("-{}", rest)) {
            return Some(vec![("transform", format!("rotateX({})", deg))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("rotate-y-") {
        if let Some(deg) = deg_val(rest) {
            return Some(vec![("transform", format!("rotateY({})", deg))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-y-") {
        if let Some(deg) = deg_val(&format!("-{}", rest)) {
            return Some(vec![("transform", format!("rotateY({})", deg))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("rotate-z-") {
        if let Some(deg) = deg_val(rest) {
            return Some(vec![("rotate", deg)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-z-") {
        if let Some(deg) = deg_val(&format!("-{}", rest)) {
            return Some(vec![("rotate", deg)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("rotate-") {
        if let Some(deg) = deg_val(rest) {
            return Some(vec![("transform", format!("rotate({})", deg))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-") {
        if let Some(deg) = deg_val(&format!("-{}", rest)) {
            return Some(vec![("transform", format!("rotate({})", deg))]);
        }
    }

    // Translate
    if class_name == "translate-none" {
        return Some(vec![("translate", "none".to_string())]);
    }
    if class_name == "translate-3d" {
        return Some(vec![("transform-style", "preserve-3d".to_string())]);
    }
    if let Some(rest) = class_name.strip_prefix("translate-x-") {
        if let Some(val) = resolve_length_val(rest) {
            return Some(vec![("transform", format!("translateX({})", val))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-translate-x-") {
        if let Some(val) = resolve_length_val(&format!("-{}", rest)) {
            return Some(vec![("transform", format!("translateX({})", val))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("translate-y-") {
        if let Some(val) = resolve_length_val(rest) {
            return Some(vec![("transform", format!("translateY({})", val))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-translate-y-") {
        if let Some(val) = resolve_length_val(&format!("-{}", rest)) {
            return Some(vec![("transform", format!("translateY({})", val))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("translate-z-") {
        if let Some(val) = resolve_length_val(rest) {
            return Some(vec![("translate", format!("0 0 {}", val))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-translate-z-") {
        if let Some(val) = resolve_length_val(&format!("-{}", rest)) {
            return Some(vec![("translate", format!("0 0 {}", val))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("translate-") {
        if let Some(val) = resolve_length_val(rest) {
            return Some(vec![("translate", val)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-translate-") {
        if let Some(val) = resolve_length_val(&format!("-{}", rest)) {
            return Some(vec![("translate", val)]);
        }
    }

    // Transition Properties
    match class_name {
        "transition" | "transition-normal" => {
            return Some(vec![
                ("transition-property", "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, opacity, box-shadow, transform, translate, scale, rotate, filter, backdrop-filter".to_string()),
                ("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)".to_string()),
                ("transition-duration", "150ms".to_string()),
            ]);
        }
        "transition-all" => {
            return Some(vec![
                ("transition-property", "all".to_string()),
                ("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)".to_string()),
                ("transition-duration", "150ms".to_string()),
            ]);
        }
        "transition-colors" => {
            return Some(vec![
                ("transition-property", "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke".to_string()),
                ("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)".to_string()),
                ("transition-duration", "150ms".to_string()),
            ]);
        }
        "transition-discrete" => {
            return Some(vec![
                ("transition-behavior", "discrete".to_string()),
                ("transition-property", "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, opacity, box-shadow, transform, translate, scale, rotate, filter, backdrop-filter".to_string()),
                ("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)".to_string()),
                ("transition-duration", "150ms".to_string()),
            ]);
        }
        "transition-none" => {
            return Some(vec![("transition-property", "none".to_string())]);
        }
        "transition-opacity" => {
            return Some(vec![
                ("transition-property", "opacity".to_string()),
                ("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)".to_string()),
                ("transition-duration", "150ms".to_string()),
            ]);
        }
        "transition-shadow" => {
            return Some(vec![
                ("transition-property", "box-shadow".to_string()),
                ("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)".to_string()),
                ("transition-duration", "150ms".to_string()),
            ]);
        }
        "transition-transform" => {
            return Some(vec![
                ("transition-property", "transform, translate, scale, rotate".to_string()),
                ("transition-timing-function", "cubic-bezier(0.4, 0, 0.2, 1)".to_string()),
                ("transition-duration", "150ms".to_string()),
            ]);
        }
        _ => {}
    }

    // Duration
    if let Some(rest) = class_name.strip_prefix("duration-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("transition-duration", format!("{}ms", n))]);
        }
    }

    // Delay
    if let Some(rest) = class_name.strip_prefix("delay-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("transition-delay", format!("{}ms", n))]);
        }
    }

    // Ease
    let ease = match class_name {
        "ease-linear" => Some("linear"),
        "ease-in" => Some("cubic-bezier(0.4, 0, 1, 1)"),
        "ease-out" => Some("cubic-bezier(0, 0, 0.2, 1)"),
        "ease-in-out" => Some("cubic-bezier(0.4, 0, 0.2, 1)"),
        _ => None,
    };
    if let Some(es) = ease {
        return Some(vec![("transition-timing-function", es.to_string())]);
    }
    // Skew
    let skew_deg = |s: &str| -> Option<String> {
        let (neg, val) = if let Some(stripped) = s.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, s)
        };
        if val.parse::<u32>().is_ok() {
            Some(if neg { format!("-{}deg", val) } else { format!("{}deg", val) })
        } else {
            None
        }
    };
    if let Some(rest) = class_name.strip_prefix("skew-x-") {
        if let Some(deg) = skew_deg(rest) {
            return Some(vec![("skew-x", deg)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-skew-x-") {
        if let Some(deg) = skew_deg(&format!("-{}", rest)) {
            return Some(vec![("skew-x", deg)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("skew-y-") {
        if let Some(deg) = skew_deg(rest) {
            return Some(vec![("skew-y", deg)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-skew-y-") {
        if let Some(deg) = skew_deg(&format!("-{}", rest)) {
            return Some(vec![("skew-y", deg)]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("skew-") {
        if let Some(deg) = skew_deg(rest) {
            return Some(vec![("transform", format!("skew({})", deg))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-skew-") {
        if let Some(deg) = skew_deg(&format!("-{}", rest)) {
            return Some(vec![("transform", format!("skew({})", deg))]);
        }
    }

    // Transform Origin
    let origin = match class_name {
        "origin-center" => Some("center"),
        "origin-top" => Some("top"),
        "origin-top-right" => Some("top right"),
        "origin-right" => Some("right"),
        "origin-bottom-right" => Some("bottom right"),
        "origin-bottom" => Some("bottom"),
        "origin-bottom-left" => Some("bottom left"),
        "origin-left" => Some("left"),
        "origin-top-left" => Some("top left"),
        _ => None,
    };
    if let Some(org) = origin {
        return Some(vec![("transform-origin", org.to_string())]);
    }

    // Perspective & Perspective Origin
    let perspective = match class_name {
        "perspective-none" => Some("none".to_string()),
        "perspective-distant" => Some("1250px".to_string()),
        "perspective-dramatic" => Some("100px".to_string()),
        "perspective-midrange" => Some("800px".to_string()),
        "perspective-near" => Some("300px".to_string()),
        "perspective-normal" => Some("500px".to_string()),
        _ => None,
    };
    if let Some(p) = perspective {
        return Some(vec![("perspective", p)]);
    }
    if let Some(rest) = class_name.strip_prefix("perspective-") {
        if !rest.starts_with("origin-") {
            if let Some(val) = resolve_length_val(rest) {
                return Some(vec![("perspective", val)]);
            }
        }
    }
    let pers_origin = match class_name {
        "perspective-origin-center" => Some("center"),
        "perspective-origin-top" => Some("top"),
        "perspective-origin-top-right" => Some("top right"),
        "perspective-origin-right" => Some("right"),
        "perspective-origin-bottom-right" => Some("bottom right"),
        "perspective-origin-bottom" => Some("bottom"),
        "perspective-origin-bottom-left" => Some("bottom left"),
        "perspective-origin-left" => Some("left"),
        "perspective-origin-top-left" => Some("top left"),
        _ => None,
    };
    if let Some(po) = pers_origin {
        return Some(vec![("perspective-origin", po.to_string())]);
    }

    // Filter & Backdrop Filter Utilities
    if let Some(rules) = super::filter::resolve_filter_rules(class_name) {
        return Some(rules);
    }

    None
}


