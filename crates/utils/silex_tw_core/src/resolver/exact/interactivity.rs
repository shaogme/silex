use std::borrow::Cow;

/// 交互、滚动与可访问性规则解析
pub fn resolve_interactivity_rules(
    class_name: &str,
) -> Option<&'static [(&'static str, Cow<'static, str>)]> {
    match class_name {
        // Accessibility
        "sr-only" => Some(cow![
            ("position", "absolute"),
            ("width", "1px"),
            ("height", "1px"),
            ("padding", "0"),
            ("margin", "-1px"),
            ("overflow", "hidden"),
            ("clip", "rect(0, 0, 0, 0)"),
            ("white-space", "nowrap"),
            ("border-width", "0"),
        ]),
        "not-sr-only" => Some(cow![
            ("position", "static"),
            ("width", "auto"),
            ("height", "auto"),
            ("padding", "0"),
            ("margin", "0"),
            ("overflow", "visible"),
            ("clip", "auto"),
            ("white-space", "normal"),
        ]),

        // Cursors
        "cursor-pointer" => Some(cow![("cursor", "pointer")]),
        "cursor-default" => Some(cow![("cursor", "default")]),
        "cursor-not-allowed" => Some(cow![("cursor", "not-allowed")]),
        "cursor-wait" => Some(cow![("cursor", "wait")]),
        "cursor-move" => Some(cow![("cursor", "move")]),
        "cursor-text" => Some(cow![("cursor", "text")]),
        "cursor-auto" => Some(cow![("cursor", "auto")]),
        "cursor-none" => Some(cow![("cursor", "none")]),
        "cursor-context-menu" => Some(cow![("cursor", "context-menu")]),
        "cursor-help" => Some(cow![("cursor", "help")]),
        "cursor-progress" => Some(cow![("cursor", "progress")]),
        "cursor-cell" => Some(cow![("cursor", "cell")]),
        "cursor-crosshair" => Some(cow![("cursor", "crosshair")]),
        "cursor-vertical-text" => Some(cow![("cursor", "vertical-text")]),
        "cursor-alias" => Some(cow![("cursor", "alias")]),
        "cursor-copy" => Some(cow![("cursor", "copy")]),
        "cursor-no-drop" => Some(cow![("cursor", "no-drop")]),
        "cursor-grab" => Some(cow![("cursor", "grab")]),
        "cursor-grabbing" => Some(cow![("cursor", "grabbing")]),
        "cursor-all-scroll" => Some(cow![("cursor", "all-scroll")]),
        "cursor-col-resize" => Some(cow![("cursor", "col-resize")]),
        "cursor-row-resize" => Some(cow![("cursor", "row-resize")]),
        "cursor-n-resize" => Some(cow![("cursor", "n-resize")]),
        "cursor-e-resize" => Some(cow![("cursor", "e-resize")]),
        "cursor-s-resize" => Some(cow![("cursor", "s-resize")]),
        "cursor-w-resize" => Some(cow![("cursor", "w-resize")]),
        "cursor-ne-resize" => Some(cow![("cursor", "ne-resize")]),
        "cursor-nw-resize" => Some(cow![("cursor", "nw-resize")]),
        "cursor-se-resize" => Some(cow![("cursor", "se-resize")]),
        "cursor-sw-resize" => Some(cow![("cursor", "sw-resize")]),
        "cursor-ew-resize" => Some(cow![("cursor", "ew-resize")]),
        "cursor-ns-resize" => Some(cow![("cursor", "ns-resize")]),
        "cursor-nesw-resize" => Some(cow![("cursor", "nesw-resize")]),
        "cursor-nwse-resize" => Some(cow![("cursor", "nwse-resize")]),
        "cursor-zoom-in" => Some(cow![("cursor", "zoom-in")]),
        "cursor-zoom-out" => Some(cow![("cursor", "zoom-out")]),

        // Pointer Events & User Select
        "pointer-events-none" => Some(cow![("pointer-events", "none")]),
        "pointer-events-auto" => Some(cow![("pointer-events", "auto")]),
        "select-none" => Some(cow![("user-select", "none")]),
        "select-text" => Some(cow![("user-select", "text")]),
        "select-all" => Some(cow![("user-select", "all")]),
        "select-auto" => Some(cow![("user-select", "auto")]),

        // Resize & Touch Action
        "resize-none" => Some(cow![("resize", "none")]),
        "resize-y" => Some(cow![("resize", "vertical")]),
        "resize-x" => Some(cow![("resize", "horizontal")]),
        "resize" => Some(cow![("resize", "both")]),
        "touch-auto" => Some(cow![("touch-action", "auto")]),
        "touch-none" => Some(cow![("touch-action", "none")]),
        "touch-pan-x" => Some(cow![("touch-action", "pan-x")]),
        "touch-pan-left" => Some(cow![("touch-action", "pan-left")]),
        "touch-pan-right" => Some(cow![("touch-action", "pan-right")]),
        "touch-pan-y" => Some(cow![("touch-action", "pan-y")]),
        "touch-pan-up" => Some(cow![("touch-action", "pan-up")]),
        "touch-pan-down" => Some(cow![("touch-action", "pan-down")]),
        "touch-pinch-zoom" => Some(cow![("touch-action", "pinch-zoom")]),
        "touch-manipulation" => Some(cow![("touch-action", "manipulation")]),

        // Scroll Behavior & Scrollbar & Color Scheme
        "scroll-smooth" => Some(cow![("scroll-behavior", "smooth")]),
        "scroll-auto" => Some(cow![("scroll-behavior", "auto")]),
        "scrollbar-auto" => Some(cow![("scrollbar-width", "auto")]),
        "scrollbar-none" => Some(cow![("scrollbar-width", "none")]),
        "scrollbar-thin" => Some(cow![("scrollbar-width", "thin")]),
        "scrollbar-gutter-auto" => Some(cow![("scrollbar-gutter", "auto")]),
        "scrollbar-gutter-stable" => Some(cow![("scrollbar-gutter", "stable")]),
        "scrollbar-gutter-both" => Some(cow![("scrollbar-gutter", "stable both-edges")]),
        "scheme-normal" => Some(cow![("color-scheme", "normal")]),
        "scheme-light" => Some(cow![("color-scheme", "light")]),
        "scheme-dark" => Some(cow![("color-scheme", "dark")]),
        "scheme-light-dark" => Some(cow![("color-scheme", "light dark")]),
        "scheme-only-light" => Some(cow![("color-scheme", "only light")]),
        "scheme-only-dark" => Some(cow![("color-scheme", "only dark")]),

        // Scroll Snap
        "snap-none" => Some(cow![("scroll-snap-type", "none")]),
        "snap-x" => Some(cow![(
            "scroll-snap-type",
            "x var(--tw-scroll-snap-strictness)"
        )]),
        "snap-y" => Some(cow![(
            "scroll-snap-type",
            "y var(--tw-scroll-snap-strictness)"
        )]),
        "snap-both" => Some(cow![(
            "scroll-snap-type",
            "both var(--tw-scroll-snap-strictness)"
        )]),
        "snap-mandatory" => Some(cow![("--tw-scroll-snap-strictness", "mandatory")]),
        "snap-proximity" => Some(cow![("--tw-scroll-snap-strictness", "proximity")]),
        "snap-start" => Some(cow![("scroll-snap-align", "start")]),
        "snap-end" => Some(cow![("scroll-snap-align", "end")]),
        "snap-center" => Some(cow![("scroll-snap-align", "center")]),
        "snap-align-none" => Some(cow![("scroll-snap-align", "none")]),
        "snap-normal" => Some(cow![("scroll-snap-stop", "normal")]),
        "snap-always" => Some(cow![("scroll-snap-stop", "always")]),

        // Vertical Align
        "align-baseline" => Some(cow![("vertical-align", "baseline")]),
        "align-top" => Some(cow![("vertical-align", "top")]),
        "align-middle" => Some(cow![("vertical-align", "middle")]),
        "align-bottom" => Some(cow![("vertical-align", "bottom")]),
        "align-text-top" => Some(cow![("vertical-align", "text-top")]),
        "align-text-bottom" => Some(cow![("vertical-align", "text-bottom")]),
        "align-sub" => Some(cow![("vertical-align", "sub")]),
        "align-super" => Some(cow![("vertical-align", "super")]),

        // Appearance
        "appearance-none" => Some(cow![("appearance", "none")]),
        "appearance-auto" => Some(cow![("appearance", "auto")]),

        // Will-Change & Forced Colors & Accent
        "will-change-auto" => Some(cow![("will-change", "auto")]),
        "will-change-scroll" => Some(cow![("will-change", "scroll-position")]),
        "will-change-contents" => Some(cow![("will-change", "contents")]),
        "will-change-transform" => Some(cow![("will-change", "transform")]),
        "forced-color-adjust-auto" => Some(cow![("forced-color-adjust", "auto")]),
        "forced-color-adjust-none" => Some(cow![("forced-color-adjust", "none")]),
        "accent-auto" => Some(cow![("accent-color", "auto")]),

        _ => None,
    }
}
