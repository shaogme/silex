use super::dynamic::resolve_length_val;
use std::borrow::Cow;

/// Transform & Transition & Animation & Gradients
pub fn resolve_transform_transition_rules(
    class_name: &str,
) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    // 静态 Transform & Transition & Animation 规则匹配
    let static_rules: Option<&'static [(&'static str, Cow<'static, str>)]> = match class_name {
        // Backface
        "backface-visible" => Some(cow![("backface-visibility", "visible")]),
        "backface-hidden" => Some(cow![("backface-visibility", "hidden")]),

        // Animate in/out
        "animate-in" => Some(cow![
            ("animation-name", "enter"),
            ("animation-duration", "150ms")
        ]),
        "animate-out" => Some(cow![
            ("animation-name", "exit"),
            ("animation-duration", "150ms")
        ]),

        // Transitions & Duration/Ease Initial
        "transition-none" => Some(cow![("transition-property", "none")]),
        "transition-all" => Some(cow![
            ("transition-property", "all"),
            (
                "transition-timing-function",
                "var(--tw-ease, var(--default-transition-timing-function))",
            ),
            (
                "transition-duration",
                "var(--tw-duration, var(--default-transition-duration))",
            ),
        ]),
        "transition" => Some(cow![
            (
                "transition-property",
                "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to, opacity, box-shadow, transform, translate, scale, rotate, filter, -webkit-backdrop-filter, backdrop-filter, display, content-visibility, overlay, pointer-events",
            ),
            (
                "transition-timing-function",
                "var(--tw-ease, var(--default-transition-timing-function))",
            ),
            (
                "transition-duration",
                "var(--tw-duration, var(--default-transition-duration))",
            ),
        ]),
        "transition-colors" => Some(cow![
            (
                "transition-property",
                "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to",
            ),
            (
                "transition-timing-function",
                "var(--tw-ease, var(--default-transition-timing-function))",
            ),
            (
                "transition-duration",
                "var(--tw-duration, var(--default-transition-duration))",
            ),
        ]),
        "transition-opacity" => Some(cow![
            ("transition-property", "opacity"),
            (
                "transition-timing-function",
                "var(--tw-ease, var(--default-transition-timing-function))",
            ),
            (
                "transition-duration",
                "var(--tw-duration, var(--default-transition-duration))",
            ),
        ]),
        "transition-shadow" => Some(cow![
            ("transition-property", "box-shadow"),
            (
                "transition-timing-function",
                "var(--tw-ease, var(--default-transition-timing-function))",
            ),
            (
                "transition-duration",
                "var(--tw-duration, var(--default-transition-duration))",
            ),
        ]),
        "transition-transform" => Some(cow![
            ("transition-property", "transform, translate, scale, rotate"),
            (
                "transition-timing-function",
                "var(--tw-ease, var(--default-transition-timing-function))",
            ),
            (
                "transition-duration",
                "var(--tw-duration, var(--default-transition-duration))",
            ),
        ]),
        "transition-discrete" => Some(cow![("transition-behavior", "allow-discrete")]),
        "duration-initial" => Some(cow![("transition-duration", "initial")]),
        "ease-initial" => Some(cow![("transition-timing-function", "initial")]),
        "transition-normal" => Some(cow![("transition-behavior", "normal")]),

        // Transform Utilities
        "transform" => Some(cow![(
            "transform",
            "var(--tw-rotate-x) var(--tw-rotate-y) var(--tw-rotate-z) var(--tw-skew-x) var(--tw-skew-y)",
        )]),
        "transform-none" => Some(cow![("transform", "none")]),
        "transform-gpu" => Some(cow![(
            "transform",
            "translate3d(var(--tw-translate-x), var(--tw-translate-y), 0) rotate(var(--tw-rotate)) skewX(var(--tw-skew-x)) skewY(var(--tw-skew-y)) scaleX(var(--tw-scale-x)) scaleY(var(--tw-scale-y))",
        )]),
        "transform-cpu" => Some(cow![(
            "transform",
            "translateX(var(--tw-translate-x)) translateY(var(--tw-translate-y)) rotate(var(--tw-rotate)) skewX(var(--tw-skew-x)) skewY(var(--tw-skew-y)) scaleX(var(--tw-scale-x)) scaleY(var(--tw-scale-y))",
        )]),
        "transform-flat" => Some(cow![("transform-style", "flat")]),
        "transform-3d" => Some(cow![("transform-style", "preserve-3d")]),
        "transform-border" => Some(cow![("transform-box", "border-box")]),
        "transform-content" => Some(cow![("transform-box", "content-box")]),
        "transform-fill" => Some(cow![("transform-box", "fill-box")]),
        "transform-stroke" => Some(cow![("transform-box", "stroke-box")]),
        "transform-view" => Some(cow![("transform-box", "view-box")]),

        _ => None,
    };
    if let Some(r) = static_rules {
        return Some(r.to_vec());
    }

    // Animations
    match class_name {
        "animate" | "animate-spin" => {
            return Some(cow!(vec![
                ("animation", "spin 1s linear infinite"),
                ("will-change", "transform"),
            ]));
        }
        "animate-ping" => {
            return Some(cow!(vec![
                ("animation", "ping 1s cubic-bezier(0, 0, 0.2, 1) infinite",),
                ("will-change", "transform, opacity"),
            ]));
        }
        "animate-pulse" => {
            return Some(cow!(vec![
                (
                    "animation",
                    "pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite",
                ),
                ("will-change", "opacity"),
            ]));
        }
        "animate-bounce" => {
            return Some(cow!(vec![
                ("animation", "bounce 1s infinite"),
                ("will-change", "transform"),
            ]));
        }
        "animate-none" => return Some(cow!(vec![("animation", "none")])),
        _ => {}
    }

    // Transition duration / delay / ease
    if let Some(rest) = class_name.strip_prefix("duration-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![("transition-duration", format!("{}ms", n))]));
    }
    if let Some(rest) = class_name.strip_prefix("delay-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![("transition-delay", format!("{}ms", n))]));
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
            return Some(cow!(vec![("transition-timing-function", t)]));
        }
    }

    // Gradients
    if class_name == "bg-none" {
        return Some(cow!(vec![("background-image", "none")]));
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
            return Some(cow!(vec![(
                "background-image",
                format!("linear-gradient({}, var(--tw-gradient-stops))", d),
            )]));
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
            Some(if neg {
                format!("-{}", r)
            } else {
                format!("{}", r)
            })
        } else {
            None
        }
    };

    if class_name == "scale-none" {
        return Some(cow!(vec![("scale", "none")]));
    }
    if class_name == "scale-3d" {
        return Some(cow!(vec![("transform-style", "preserve-3d")]));
    }
    if let Some(rest) = class_name.strip_prefix("scale-x-")
        && let Some(r) = scale_ratio(rest)
    {
        return Some(cow!(vec![("scale", format!("{} 1", r))]));
    }
    if let Some(rest) = class_name.strip_prefix("-scale-x-")
        && let Some(r) = scale_ratio(&format!("-{}", rest))
    {
        return Some(cow!(vec![("scale", format!("{} 1", r))]));
    }
    if let Some(rest) = class_name.strip_prefix("scale-y-")
        && let Some(r) = scale_ratio(rest)
    {
        return Some(cow!(vec![("scale", format!("1 {}", r))]));
    }
    if let Some(rest) = class_name.strip_prefix("-scale-y-")
        && let Some(r) = scale_ratio(&format!("-{}", rest))
    {
        return Some(cow!(vec![("scale", format!("1 {}", r))]));
    }
    if let Some(rest) = class_name.strip_prefix("scale-z-")
        && let Some(r) = scale_ratio(rest)
    {
        return Some(cow!(vec![("scale", format!("1 1 {}", r))]));
    }
    if let Some(rest) = class_name.strip_prefix("-scale-z-")
        && let Some(r) = scale_ratio(&format!("-{}", rest))
    {
        return Some(cow!(vec![("scale", format!("1 1 {}", r))]));
    }
    if let Some(rest) = class_name.strip_prefix("scale-")
        && let Some(r) = scale_ratio(rest)
    {
        return Some(cow!(vec![("scale", r)]));
    }
    if let Some(rest) = class_name.strip_prefix("-scale-")
        && let Some(r) = scale_ratio(&format!("-{}", rest))
    {
        return Some(cow!(vec![("scale", r)]));
    }

    // Rotate
    let deg_val = |s: &str| -> Option<String> {
        let (neg, val) = if let Some(stripped) = s.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, s)
        };
        if val.parse::<u32>().is_ok() {
            Some(if neg {
                format!("-{}deg", val)
            } else {
                format!("{}deg", val)
            })
        } else {
            None
        }
    };
    if class_name == "rotate-none" {
        return Some(cow!(vec![("rotate", "none")]));
    }
    if let Some(rest) = class_name.strip_prefix("rotate-x-")
        && let Some(deg) = deg_val(rest)
    {
        return Some(cow!(vec![("transform", format!("rotateX({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-x-")
        && let Some(deg) = deg_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("rotateX({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("rotate-y-")
        && let Some(deg) = deg_val(rest)
    {
        return Some(cow!(vec![("transform", format!("rotateY({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-y-")
        && let Some(deg) = deg_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("rotateY({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("rotate-z-")
        && let Some(deg) = deg_val(rest)
    {
        return Some(cow!(vec![("rotate", deg)]));
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-z-")
        && let Some(deg) = deg_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("rotate", deg)]));
    }
    if let Some(rest) = class_name.strip_prefix("rotate-")
        && let Some(deg) = deg_val(rest)
    {
        return Some(cow!(vec![("transform", format!("rotate({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("-rotate-")
        && let Some(deg) = deg_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("rotate({})", deg))]));
    }

    // Translate
    if class_name == "translate-none" {
        return Some(cow!(vec![("translate", "none")]));
    }
    if class_name == "translate-3d" {
        return Some(cow!(vec![("transform-style", "preserve-3d")]));
    }
    if let Some(rest) = class_name.strip_prefix("translate-x-")
        && let Some(val) = resolve_length_val(rest)
    {
        return Some(cow!(vec![("transform", format!("translateX({})", val))]));
    }
    if let Some(rest) = class_name.strip_prefix("-translate-x-")
        && let Some(val) = resolve_length_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("translateX({})", val))]));
    }
    if let Some(rest) = class_name.strip_prefix("translate-y-")
        && let Some(val) = resolve_length_val(rest)
    {
        return Some(cow!(vec![("transform", format!("translateY({})", val))]));
    }
    if let Some(rest) = class_name.strip_prefix("-translate-y-")
        && let Some(val) = resolve_length_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("translateY({})", val))]));
    }
    if let Some(rest) = class_name.strip_prefix("translate-z-")
        && let Some(val) = resolve_length_val(rest)
    {
        return Some(cow!(vec![("translate", format!("0 0 {}", val))]));
    }
    if let Some(rest) = class_name.strip_prefix("-translate-z-")
        && let Some(val) = resolve_length_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("translate", format!("0 0 {}", val))]));
    }
    if let Some(rest) = class_name.strip_prefix("translate-")
        && let Some(val) = resolve_length_val(rest)
    {
        return Some(cow!(vec![("translate", val)]));
    }
    if let Some(rest) = class_name.strip_prefix("-translate-")
        && let Some(val) = resolve_length_val(&format!("-{}", rest))
    {
        return Some(cow!(vec![("translate", val)]));
    }

    // Duration
    if let Some(rest) = class_name.strip_prefix("duration-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![("transition-duration", format!("{}ms", n))]));
    }

    // Delay
    if let Some(rest) = class_name.strip_prefix("delay-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec![("transition-delay", format!("{}ms", n))]));
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
        return Some(cow!(vec![("transition-timing-function", es)]));
    }
    // Skew
    let skew_deg = |s: &str| -> Option<String> {
        let (neg, val) = if let Some(stripped) = s.strip_prefix('-') {
            (true, stripped)
        } else {
            (false, s)
        };
        if val.parse::<u32>().is_ok() {
            Some(if neg {
                format!("-{}deg", val)
            } else {
                format!("{}deg", val)
            })
        } else {
            None
        }
    };
    if let Some(rest) = class_name.strip_prefix("skew-x-")
        && let Some(deg) = skew_deg(rest)
    {
        return Some(cow!(vec![("transform", format!("skewX({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("-skew-x-")
        && let Some(deg) = skew_deg(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("skewX({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("skew-y-")
        && let Some(deg) = skew_deg(rest)
    {
        return Some(cow!(vec![("transform", format!("skewY({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("-skew-y-")
        && let Some(deg) = skew_deg(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("skewY({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("skew-")
        && let Some(deg) = skew_deg(rest)
    {
        return Some(cow!(vec![("transform", format!("skew({})", deg))]));
    }
    if let Some(rest) = class_name.strip_prefix("-skew-")
        && let Some(deg) = skew_deg(&format!("-{}", rest))
    {
        return Some(cow!(vec![("transform", format!("skew({})", deg))]));
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
        return Some(cow!(vec![("transform-origin", org)]));
    }

    // Perspective & Perspective Origin
    let perspective = match class_name {
        "perspective-none" => Some("none"),
        "perspective-distant" => Some("1200px"),
        "perspective-dramatic" => Some("100px"),
        "perspective-midrange" => Some("800px"),
        "perspective-near" => Some("300px"),
        "perspective-normal" => Some("500px"),
        _ => None,
    };
    if let Some(p) = perspective {
        return Some(cow!(vec![("perspective", p)]));
    }
    if let Some(rest) = class_name.strip_prefix("perspective-")
        && !rest.starts_with("origin-")
        && let Some(val) = resolve_length_val(rest)
    {
        return Some(cow!(vec![("perspective", val)]));
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
        return Some(cow!(vec![("perspective-origin", po)]));
    }

    // Filter & Backdrop Filter Utilities
    if let Some(rules) = super::filter::resolve_filter_rules(class_name) {
        return Some(rules);
    }

    None
}
