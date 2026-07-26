use std::borrow::Cow;

/// 遮罩 (Mask) 与 渐变方向 (Gradient Directions) 静态规则解析
pub fn resolve_mask_rules(
    class_name: &str,
) -> Option<&'static [(&'static str, Cow<'static, str>)]> {
    match class_name {
        // Background Gradient Directions & Conic/Radial Base
        "bg" => Some(cow![("background-color", "transparent")]),
        "bg-none" => Some(cow![("background-image", "none")]),
        "bg-linear-to-t" => Some(cow![(
            "background-image",
            "linear-gradient(to top, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-tr" => Some(cow![(
            "background-image",
            "linear-gradient(to top right, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-r" => Some(cow![(
            "background-image",
            "linear-gradient(to right, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-br" => Some(cow![(
            "background-image",
            "linear-gradient(to bottom right, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-b" => Some(cow![(
            "background-image",
            "linear-gradient(to bottom, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-bl" => Some(cow![(
            "background-image",
            "linear-gradient(to bottom left, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-l" => Some(cow![(
            "background-image",
            "linear-gradient(to left, var(--tw-gradient-stops))",
        )]),
        "bg-linear-to-tl" => Some(cow![(
            "background-image",
            "linear-gradient(to top left, var(--tw-gradient-stops))",
        )]),
        "bg-radial" => Some(cow![(
            "background-image",
            "radial-gradient(var(--tw-gradient-stops))",
        )]),
        "bg-conic" => Some(cow![(
            "background-image",
            "conic-gradient(var(--tw-gradient-stops))",
        )]),
        "via-none" => Some(cow![("--tw-gradient-via-stops", "none")]),

        // Background Attachment / Clip / Origin / Repeat / Size / Position
        "bg-fixed" => Some(cow![("background-attachment", "fixed")]),
        "bg-local" => Some(cow![("background-attachment", "local")]),
        "bg-scroll" => Some(cow![("background-attachment", "scroll")]),
        "bg-clip-border" => Some(cow![("background-clip", "border-box")]),
        "bg-clip-padding" => Some(cow![("background-clip", "padding-box")]),
        "bg-clip-content" => Some(cow![("background-clip", "content-box")]),
        "bg-clip-text" => Some(cow![("background-clip", "text")]),
        "bg-origin-border" => Some(cow![("background-origin", "border-box")]),
        "bg-origin-padding" => Some(cow![("background-origin", "padding-box")]),
        "bg-origin-content" => Some(cow![("background-origin", "content-box")]),
        "bg-repeat" => Some(cow![("background-repeat", "repeat")]),
        "bg-no-repeat" => Some(cow![("background-repeat", "no-repeat")]),
        "bg-repeat-x" => Some(cow![("background-repeat", "repeat-x")]),
        "bg-repeat-y" => Some(cow![("background-repeat", "repeat-y")]),
        "bg-repeat-round" => Some(cow![("background-repeat", "round")]),
        "bg-repeat-space" => Some(cow![("background-repeat", "space")]),
        "bg-auto" => Some(cow![("background-size", "auto")]),
        "bg-cover" => Some(cow![("background-size", "cover")]),
        "bg-contain" => Some(cow![("background-size", "contain")]),
        "bg-bottom" => Some(cow![("background-position", "bottom")]),
        "bg-center" => Some(cow![("background-position", "center")]),
        "bg-left" => Some(cow![("background-position", "left")]),
        "bg-right" => Some(cow![("background-position", "right")]),
        "bg-top" => Some(cow![("background-position", "top")]),
        "bg-top-left" => Some(cow![("background-position", "top left")]),
        "bg-top-right" => Some(cow![("background-position", "top right")]),
        "bg-bottom-left" => Some(cow![("background-position", "bottom left")]),
        "bg-bottom-right" => Some(cow![("background-position", "bottom right")]),
        // v3 的轴序写法，v4 仍然保留为别名
        "bg-left-top" => Some(cow![("background-position", "top left")]),
        "bg-right-top" => Some(cow![("background-position", "top right")]),
        "bg-left-bottom" => Some(cow![("background-position", "bottom left")]),
        "bg-right-bottom" => Some(cow![("background-position", "bottom right")]),

        // Mask Base & Position & Mode
        "mask-none" => Some(cow![("mask-image", "none")]),
        "mask-alpha" => Some(cow![("mask-type", "alpha")]),
        "mask-type-alpha" => Some(cow![("mask-type", "alpha")]),
        "mask-type-luminance" => Some(cow![("mask-type", "luminance")]),
        "mask-luminance" => Some(cow![("mask-type", "luminance")]),
        "mask-match" => Some(cow![("mask-mode", "match-source")]),
        "mask-circle" => Some(cow![(
            "mask-image",
            "radial-gradient(circle, var(--tw-mask-stops))",
        )]),
        "mask-ellipse" => Some(cow![(
            "mask-image",
            "radial-gradient(ellipse, var(--tw-mask-stops))",
        )]),
        "mask-contain" => Some(cow![("mask-size", "contain")]),
        "mask-cover" => Some(cow![("mask-size", "cover")]),
        "mask-auto" => Some(cow![("mask-size", "auto")]),
        "mask-clip-border" => Some(cow![("mask-clip", "border-box")]),
        "mask-clip-content" => Some(cow![("mask-clip", "content-box")]),
        "mask-clip-padding" => Some(cow![("mask-clip", "padding-box")]),
        "mask-clip-fill" => Some(cow![("mask-clip", "fill-box")]),
        "mask-clip-stroke" => Some(cow![("mask-clip", "stroke-box")]),
        "mask-clip-view" => Some(cow![("mask-clip", "view-box")]),
        "mask-no-clip" => Some(cow![("mask-clip", "no-clip")]),
        "mask-origin-border" => Some(cow![("mask-origin", "border-box")]),
        "mask-origin-content" => Some(cow![("mask-origin", "content-box")]),
        "mask-origin-padding" => Some(cow![("mask-origin", "padding-box")]),
        "mask-origin-fill" => Some(cow![("mask-origin", "fill-box")]),
        "mask-origin-stroke" => Some(cow![("mask-origin", "stroke-box")]),
        "mask-origin-view" => Some(cow![("mask-origin", "view-box")]),
        "mask-repeat" => Some(cow![("mask-repeat", "repeat")]),
        "mask-no-repeat" => Some(cow![("mask-repeat", "no-repeat")]),
        "mask-repeat-round" => Some(cow![("mask-repeat", "round")]),
        "mask-repeat-space" => Some(cow![("mask-repeat", "space")]),
        "mask-repeat-x" => Some(cow![("mask-repeat", "repeat-x")]),
        "mask-repeat-y" => Some(cow![("mask-repeat", "repeat-y")]),
        "mask-top" => Some(cow![("mask-position", "top")]),
        "mask-bottom" => Some(cow![("mask-position", "bottom")]),
        "mask-left" => Some(cow![("mask-position", "left")]),
        "mask-right" => Some(cow![("mask-position", "right")]),
        "mask-center" => Some(cow![("mask-position", "center")]),
        "mask-top-left" => Some(cow![("mask-position", "top left")]),
        "mask-top-right" => Some(cow![("mask-position", "top right")]),
        "mask-bottom-left" => Some(cow![("mask-position", "bottom left")]),
        "mask-bottom-right" => Some(cow![("mask-position", "bottom right")]),
        "mask-add" => Some(cow![("mask-composite", "add")]),
        "mask-subtract" => Some(cow![("mask-composite", "subtract")]),
        "mask-intersect" => Some(cow![("mask-composite", "intersect")]),
        "mask-exclude" => Some(cow![("mask-composite", "exclude")]),
        "mask-radial-at-bottom" => Some(cow![(
            "mask-image",
            "radial-gradient(at bottom, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-bottom-left" => Some(cow![(
            "mask-image",
            "radial-gradient(at bottom left, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-bottom-right" => Some(cow![(
            "mask-image",
            "radial-gradient(at bottom right, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-center" => Some(cow![(
            "mask-image",
            "radial-gradient(at center, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-left" => Some(cow![(
            "mask-image",
            "radial-gradient(at left, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-right" => Some(cow![(
            "mask-image",
            "radial-gradient(at right, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-top" => Some(cow![(
            "mask-image",
            "radial-gradient(at top, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-top-left" => Some(cow![(
            "mask-image",
            "radial-gradient(at top left, var(--tw-mask-stops))",
        )]),
        "mask-radial-at-top-right" => Some(cow![(
            "mask-image",
            "radial-gradient(at top right, var(--tw-mask-stops))",
        )]),
        "mask-radial-closest-corner" => Some(cow![(
            "mask-image",
            "radial-gradient(closest-corner, var(--tw-mask-stops))",
        )]),
        "mask-radial-closest-side" => Some(cow![(
            "mask-image",
            "radial-gradient(closest-side, var(--tw-mask-stops))",
        )]),
        "mask-radial-farthest-corner" => Some(cow![(
            "mask-image",
            "radial-gradient(farthest-corner, var(--tw-mask-stops))",
        )]),
        "mask-radial-farthest-side" => Some(cow![(
            "mask-image",
            "radial-gradient(farthest-side, var(--tw-mask-stops))",
        )]),

        _ => None,
    }
}
