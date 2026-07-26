use std::borrow::Cow;

/// 布局、定位与容器规则解析
pub fn resolve_layout_rules(
    class_name: &str,
) -> Option<&'static [(&'static str, Cow<'static, str>)]> {
    match class_name {
        // Layout & Display
        "block" => Some(cow![("display", "block")]),
        "inline-block" => Some(cow![("display", "inline-block")]),
        "inline" => Some(cow![("display", "inline")]),
        "flex" => Some(cow![("display", "flex")]),
        "inline-flex" => Some(cow![("display", "inline-flex")]),
        "grid" => Some(cow![("display", "grid")]),
        "inline-grid" => Some(cow![("display", "inline-grid")]),
        "hidden" => Some(cow![("display", "none")]),
        "contents" => Some(cow![("display", "contents")]),
        "table" => Some(cow![("display", "table")]),
        "table-row" => Some(cow![("display", "table-row")]),
        "table-cell" => Some(cow![("display", "table-cell")]),
        "flow-root" => Some(cow![("display", "flow-root")]),
        "inline-table" => Some(cow![("display", "inline-table")]),

        // Position
        "static" => Some(cow![("position", "static")]),
        "fixed" => Some(cow![("position", "fixed")]),
        "absolute" => Some(cow![("position", "absolute")]),
        "relative" => Some(cow![("position", "relative")]),
        "sticky" => Some(cow![("position", "sticky")]),

        // Visibility
        "visible" => Some(cow![("visibility", "visible")]),
        "invisible" => Some(cow![("visibility", "hidden")]),
        "collapse" => Some(cow![("visibility", "collapse")]),

        // Overflow & Overscroll
        "overflow-auto" => Some(cow![("overflow", "auto")]),
        "overflow-hidden" => Some(cow![("overflow", "hidden")]),
        "overflow-visible" => Some(cow![("overflow", "visible")]),
        "overflow-scroll" => Some(cow![("overflow", "scroll")]),
        "overflow-x-auto" => Some(cow![("overflow-x", "auto")]),
        "overflow-x-hidden" => Some(cow![("overflow-x", "hidden")]),
        "overflow-y-auto" => Some(cow![("overflow-y", "auto")]),
        "overflow-y-hidden" => Some(cow![("overflow-y", "hidden")]),
        "overflow-clip" => Some(cow![("overflow", "clip")]),
        "overflow-x-clip" => Some(cow![("overflow-x", "clip")]),
        "overflow-y-clip" => Some(cow![("overflow-y", "clip")]),
        "overflow-x-scroll" => Some(cow![("overflow-x", "scroll")]),
        "overflow-y-scroll" => Some(cow![("overflow-y", "scroll")]),
        "overflow-x-visible" => Some(cow![("overflow-x", "visible")]),
        "overflow-y-visible" => Some(cow![("overflow-y", "visible")]),

        "overscroll-auto" => Some(cow![("overscroll-behavior", "auto")]),
        "overscroll-contain" => Some(cow![("overscroll-behavior", "contain")]),
        "overscroll-none" => Some(cow![("overscroll-behavior", "none")]),
        "overscroll-x-auto" => Some(cow![("overscroll-behavior-x", "auto")]),
        "overscroll-x-contain" => Some(cow![("overscroll-behavior-x", "contain")]),
        "overscroll-x-none" => Some(cow![("overscroll-behavior-x", "none")]),
        "overscroll-y-auto" => Some(cow![("overscroll-behavior-y", "auto")]),
        "overscroll-y-contain" => Some(cow![("overscroll-behavior-y", "contain")]),
        "overscroll-y-none" => Some(cow![("overscroll-behavior-y", "none")]),

        // Object fit & position
        "object-contain" => Some(cow![("object-fit", "contain")]),
        "object-cover" => Some(cow![("object-fit", "cover")]),
        "object-fill" => Some(cow![("object-fit", "fill")]),
        "object-none" => Some(cow![("object-fit", "none")]),
        "object-scale-down" => Some(cow![("object-fit", "scale-down")]),
        "object-bottom" => Some(cow![("object-position", "bottom")]),
        "object-center" => Some(cow![("object-position", "center")]),
        "object-left" => Some(cow![("object-position", "left")]),
        "object-right" => Some(cow![("object-position", "right")]),
        "object-top" => Some(cow![("object-position", "top")]),
        "object-top-left" => Some(cow![("object-position", "top left")]),
        "object-top-right" => Some(cow![("object-position", "top right")]),
        "object-bottom-left" => Some(cow![("object-position", "bottom left")]),
        "object-bottom-right" => Some(cow![("object-position", "bottom right")]),

        // Aspect ratio
        "aspect-auto" => Some(cow![("aspect-ratio", "auto")]),
        "aspect-square" => Some(cow![("aspect-ratio", "1 / 1")]),
        "aspect-video" => Some(cow![("aspect-ratio", "16 / 9")]),

        // Box Sizing & Decoration
        "box-border" => Some(cow![("box-sizing", "border-box")]),
        "box-content" => Some(cow![("box-sizing", "content-box")]),
        "box-decoration-clone" => Some(cow![("box-decoration-break", "clone")]),
        "box-decoration-slice" => Some(cow![("box-decoration-break", "slice")]),

        // Clear & Float
        "clear-left" => Some(cow![("clear", "left")]),
        "clear-right" => Some(cow![("clear", "right")]),
        "clear-both" => Some(cow![("clear", "both")]),
        "clear-none" => Some(cow![("clear", "none")]),
        "clear-start" => Some(cow![("clear", "inline-start")]),
        "clear-end" => Some(cow![("clear", "inline-end")]),
        "float-left" => Some(cow![("float", "left")]),
        "float-right" => Some(cow![("float", "right")]),
        "float-none" => Some(cow![("float", "none")]),
        "float-start" => Some(cow![("float", "inline-start")]),
        "float-end" => Some(cow![("float", "inline-end")]),

        // Isolation
        "isolate" => Some(cow![("isolation", "isolate")]),
        "isolation-auto" => Some(cow![("isolation", "auto")]),

        // Breaks
        "break-after-auto" => Some(cow![("break-after", "auto")]),
        "break-after-avoid" => Some(cow![("break-after", "avoid")]),
        "break-after-all" => Some(cow![("break-after", "all")]),
        "break-after-avoid-page" => Some(cow![("break-after", "avoid-page")]),
        "break-after-page" => Some(cow![("break-after", "page")]),
        "break-after-left" => Some(cow![("break-after", "left")]),
        "break-after-right" => Some(cow![("break-after", "right")]),
        "break-after-column" => Some(cow![("break-after", "column")]),
        "break-before-auto" => Some(cow![("break-before", "auto")]),
        "break-before-avoid" => Some(cow![("break-before", "avoid")]),
        "break-before-all" => Some(cow![("break-before", "all")]),
        "break-before-avoid-page" => Some(cow![("break-before", "avoid-page")]),
        "break-before-page" => Some(cow![("break-before", "page")]),
        "break-before-left" => Some(cow![("break-before", "left")]),
        "break-before-right" => Some(cow![("break-before", "right")]),
        "break-before-column" => Some(cow![("break-before", "column")]),
        "break-inside-auto" => Some(cow![("break-inside", "auto")]),
        "break-inside-avoid" => Some(cow![("break-inside", "avoid")]),
        "break-inside-avoid-page" => Some(cow![("break-inside", "avoid-page")]),
        "break-inside-avoid-column" => Some(cow![("break-inside", "avoid-column")]),
        "break-inside-avoid-flex" => Some(cow![("break-inside", "avoid-flex")]),

        // Contain & Container & Content
        "contain-none" => Some(cow![("contain", "none")]),
        "contain-strict" => Some(cow![("contain", "strict")]),
        "contain-content" => Some(cow![("contain", "content")]),
        "contain-size" => Some(cow![("contain", "size")]),
        "contain-inline-size" => Some(cow![("contain", "inline-size")]),
        "contain-layout" => Some(cow![("contain", "layout")]),
        "contain-paint" => Some(cow![("contain", "paint")]),
        "contain-style" => Some(cow![("contain", "style")]),
        "container" | "@container" => Some(cow![("container-type", "inline-size")]),
        "@container-normal" => Some(cow![("container-type", "normal")]),
        "content-none" => Some(cow![("content", "none")]),

        // Field-Sizing
        "field-sizing-content" => Some(cow![("field-sizing", "content")]),
        "field-sizing-fixed" => Some(cow![("field-sizing", "fixed")]),

        _ => None,
    }
}
