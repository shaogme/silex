/// 遮罩 (Mask) 与 渐变方向 (Gradient Directions) 静态规则解析
pub fn resolve_mask_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    let rules: Option<&'static [(&'static str, &'static str)]> = match class_name {
        // Background Gradient Directions & Conic/Radial Base
        "bg" => Some(&[("background-color", "transparent")]),
        "bg-none" => Some(&[("background-image", "none")]),
        "bg-linear-to-t" => Some(&[(
            "background-image",
            "linear-gradient(to top, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-tr" => Some(&[(
            "background-image",
            "linear-gradient(to top right, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-r" => Some(&[(
            "background-image",
            "linear-gradient(to right, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-br" => Some(&[(
            "background-image",
            "linear-gradient(to bottom right, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-b" => Some(&[(
            "background-image",
            "linear-gradient(to bottom, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-bl" => Some(&[(
            "background-image",
            "linear-gradient(to bottom left, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-l" => Some(&[(
            "background-image",
            "linear-gradient(to left, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-tl" => Some(&[(
            "background-image",
            "linear-gradient(to top left, var(--tw-gradient-stops))",
        )]),
        "bg-radial" => Some(&[(
            "background-image",
            "radial-gradient(var(--tw-gradient-stops))",
        )]),
        "bg-conic" => Some(&[(
            "background-image",
            "conic-gradient(var(--tw-gradient-stops))",
        )]),
        "via-none" => Some(&[("--tw-gradient-via-stops", "none")]),

        // Background Attachment / Clip / Origin / Repeat / Size / Position
        "bg-fixed" => Some(&[("background-attachment", "fixed")]),
        "bg-local" => Some(&[("background-attachment", "local")]),
        "bg-scroll" => Some(&[("background-attachment", "scroll")]),
        "bg-clip-border" => Some(&[("background-clip", "border-box")]),
        "bg-clip-padding" => Some(&[("background-clip", "padding-box")]),
        "bg-clip-content" => Some(&[("background-clip", "content-box")]),
        "bg-clip-text" => Some(&[("background-clip", "text")]),
        "bg-origin-border" => Some(&[("background-origin", "border-box")]),
        "bg-origin-padding" => Some(&[("background-origin", "padding-box")]),
        "bg-origin-content" => Some(&[("background-origin", "content-box")]),
        "bg-repeat" => Some(&[("background-repeat", "repeat")]),
        "bg-no-repeat" => Some(&[("background-repeat", "no-repeat")]),
        "bg-repeat-x" => Some(&[("background-repeat", "repeat-x")]),
        "bg-repeat-y" => Some(&[("background-repeat", "repeat-y")]),
        "bg-repeat-round" => Some(&[("background-repeat", "round")]),
        "bg-repeat-space" => Some(&[("background-repeat", "space")]),
        "bg-auto" => Some(&[("background-size", "auto")]),
        "bg-cover" => Some(&[("background-size", "cover")]),
        "bg-contain" => Some(&[("background-size", "contain")]),
        "bg-bottom" => Some(&[("background-position", "bottom")]),
        "bg-center" => Some(&[("background-position", "center")]),
        "bg-left" => Some(&[("background-position", "left")]),
        "bg-right" => Some(&[("background-position", "right")]),
        "bg-top" => Some(&[("background-position", "top")]),
        "bg-top-left" => Some(&[("background-position", "top left")]),
        "bg-top-right" => Some(&[("background-position", "top right")]),
        "bg-bottom-left" => Some(&[("background-position", "bottom left")]),
        "bg-bottom-right" => Some(&[("background-position", "bottom right")]),

        // Mask Base & Position & Mode
        "mask-none" => Some(&[("mask-image", "none")]),
        "mask-alpha" => Some(&[("mask-type", "alpha")]),
        "mask-type-alpha" => Some(&[("mask-type", "alpha")]),
        "mask-type-luminance" => Some(&[("mask-type", "luminance")]),
        "mask-luminance" => Some(&[("mask-type", "luminance")]),
        "mask-match" => Some(&[("mask-mode", "match-source")]),
        "mask-circle" => Some(&[(
            "mask-image",
            "radial-gradient(circle, var(--tw-mask-stops))",
        )]),
        "mask-ellipse" => Some(&[(
            "mask-image",
            "radial-gradient(ellipse, var(--tw-mask-stops))",
        )]),
        "mask-contain" => Some(&[("mask-size", "contain")]),
        "mask-cover" => Some(&[("mask-size", "cover")]),
        "mask-auto" => Some(&[("mask-size", "auto")]),
        "mask-clip-border" => Some(&[("mask-clip", "border-box")]),
        "mask-clip-content" => Some(&[("mask-clip", "content-box")]),
        "mask-clip-padding" => Some(&[("mask-clip", "padding-box")]),
        "mask-clip-fill" => Some(&[("mask-clip", "fill-box")]),
        "mask-clip-stroke" => Some(&[("mask-clip", "stroke-box")]),
        "mask-clip-view" => Some(&[("mask-clip", "view-box")]),
        "mask-no-clip" => Some(&[("mask-clip", "no-clip")]),
        "mask-origin-border" => Some(&[("mask-origin", "border-box")]),
        "mask-origin-content" => Some(&[("mask-origin", "content-box")]),
        "mask-origin-padding" => Some(&[("mask-origin", "padding-box")]),
        "mask-origin-fill" => Some(&[("mask-origin", "fill-box")]),
        "mask-origin-stroke" => Some(&[("mask-origin", "stroke-box")]),
        "mask-origin-view" => Some(&[("mask-origin", "view-box")]),
        "mask-repeat" => Some(&[("mask-repeat", "repeat")]),
        "mask-no-repeat" => Some(&[("mask-repeat", "no-repeat")]),
        "mask-repeat-round" => Some(&[("mask-repeat", "round")]),
        "mask-repeat-space" => Some(&[("mask-repeat", "space")]),
        "mask-repeat-x" => Some(&[("mask-repeat", "repeat-x")]),
        "mask-repeat-y" => Some(&[("mask-repeat", "repeat-y")]),
        "mask-top" => Some(&[("mask-position", "top")]),
        "mask-bottom" => Some(&[("mask-position", "bottom")]),
        "mask-left" => Some(&[("mask-position", "left")]),
        "mask-right" => Some(&[("mask-position", "right")]),
        "mask-center" => Some(&[("mask-position", "center")]),
        "mask-top-left" => Some(&[("mask-position", "top left")]),
        "mask-top-right" => Some(&[("mask-position", "top right")]),
        "mask-bottom-left" => Some(&[("mask-position", "bottom left")]),
        "mask-bottom-right" => Some(&[("mask-position", "bottom right")]),
        "mask-add" => Some(&[("mask-composite", "add")]),
        "mask-subtract" => Some(&[("mask-composite", "subtract")]),
        "mask-intersect" => Some(&[("mask-composite", "intersect")]),
        "mask-exclude" => Some(&[("mask-composite", "exclude")]),
        "mask-radial-at-bottom" => Some(&[(
            "mask-image",
            "radial-gradient(at bottom, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-bottom-left" => Some(&[(
            "mask-image",
            "radial-gradient(at bottom left, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-bottom-right" => Some(&[(
            "mask-image",
            "radial-gradient(at bottom right, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-center" => Some(&[(
            "mask-image",
            "radial-gradient(at center, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-left" => Some(&[(
            "mask-image",
            "radial-gradient(at left, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-right" => Some(&[(
            "mask-image",
            "radial-gradient(at right, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-top" => Some(&[(
            "mask-image",
            "radial-gradient(at top, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-top-left" => Some(&[(
            "mask-image",
            "radial-gradient(at top left, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-top-right" => Some(&[(
            "mask-image",
            "radial-gradient(at top right, var(--tw-mask-stops))",
        )]),
        "mask-radial-closest-corner" => Some(&[(
            "mask-image",
            "radial-gradient(closest-corner, var(--tw-mask-stops))",
        )]),
        "mask-radial-closest-side" => Some(&[(
            "mask-image",
            "radial-gradient(closest-side, var(--tw-mask-stops))",
        )]),
        "mask-radial-farthest-corner" => Some(&[(
            "mask-image",
            "radial-gradient(farthest-corner, var(--tw-mask-stops))",
        )]),
        "mask-radial-farthest-side" => Some(&[(
            "mask-image",
            "radial-gradient(farthest-side, var(--tw-mask-stops))",
        )]),

        _ => None,
    };

    rules.map(|r| r.iter().map(|&(k, v)| (k, v.to_string())).collect())
}
