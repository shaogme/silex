/// 布局、定位与容器规则解析
pub fn resolve_layout_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    let rules: Option<&'static [(&'static str, &'static str)]> = match class_name {
        // Layout & Display
        "block" => Some(&[("display", "block")]),
        "inline-block" => Some(&[("display", "inline-block")]),
        "inline" => Some(&[("display", "inline")]),
        "flex" => Some(&[("display", "flex")]),
        "inline-flex" => Some(&[("display", "inline-flex")]),
        "grid" => Some(&[("display", "grid")]),
        "inline-grid" => Some(&[("display", "inline-grid")]),
        "hidden" => Some(&[("display", "none")]),
        "contents" => Some(&[("display", "contents")]),
        "table" => Some(&[("display", "table")]),
        "table-row" => Some(&[("display", "table-row")]),
        "table-cell" => Some(&[("display", "table-cell")]),
        "flow-root" => Some(&[("display", "flow-root")]),
        "inline-table" => Some(&[("display", "inline-table")]),

        // Position
        "static" => Some(&[("position", "static")]),
        "fixed" => Some(&[("position", "fixed")]),
        "absolute" => Some(&[("position", "absolute")]),
        "relative" => Some(&[("position", "relative")]),
        "sticky" => Some(&[("position", "sticky")]),

        // Visibility
        "visible" => Some(&[("visibility", "visible")]),
        "invisible" => Some(&[("visibility", "hidden")]),
        "collapse" => Some(&[("visibility", "collapse")]),

        // Overflow & Overscroll
        "overflow-auto" => Some(&[("overflow", "auto")]),
        "overflow-hidden" => Some(&[("overflow", "hidden")]),
        "overflow-visible" => Some(&[("overflow", "visible")]),
        "overflow-scroll" => Some(&[("overflow", "scroll")]),
        "overflow-x-auto" => Some(&[("overflow-x", "auto")]),
        "overflow-x-hidden" => Some(&[("overflow-x", "hidden")]),
        "overflow-y-auto" => Some(&[("overflow-y", "auto")]),
        "overflow-y-hidden" => Some(&[("overflow-y", "hidden")]),
        "overflow-clip" => Some(&[("overflow", "clip")]),
        "overflow-x-clip" => Some(&[("overflow-x", "clip")]),
        "overflow-y-clip" => Some(&[("overflow-y", "clip")]),
        "overflow-x-scroll" => Some(&[("overflow-x", "scroll")]),
        "overflow-y-scroll" => Some(&[("overflow-y", "scroll")]),
        "overflow-x-visible" => Some(&[("overflow-x", "visible")]),
        "overflow-y-visible" => Some(&[("overflow-y", "visible")]),

        "overscroll-auto" => Some(&[("overscroll-behavior", "auto")]),
        "overscroll-contain" => Some(&[("overscroll-behavior", "contain")]),
        "overscroll-none" => Some(&[("overscroll-behavior", "none")]),
        "overscroll-x-auto" => Some(&[("overscroll-behavior-x", "auto")]),
        "overscroll-x-contain" => Some(&[("overscroll-behavior-x", "contain")]),
        "overscroll-x-none" => Some(&[("overscroll-behavior-x", "none")]),
        "overscroll-y-auto" => Some(&[("overscroll-behavior-y", "auto")]),
        "overscroll-y-contain" => Some(&[("overscroll-behavior-y", "contain")]),
        "overscroll-y-none" => Some(&[("overscroll-behavior-y", "none")]),

        // Object fit & position
        "object-contain" => Some(&[("object-fit", "contain")]),
        "object-cover" => Some(&[("object-fit", "cover")]),
        "object-fill" => Some(&[("object-fill", "fill")]),
        "object-none" => Some(&[("object-fit", "none")]),
        "object-scale-down" => Some(&[("object-fit", "scale-down")]),
        "object-bottom" => Some(&[("object-position", "bottom")]),
        "object-center" => Some(&[("object-position", "center")]),
        "object-left" => Some(&[("object-position", "left")]),
        "object-right" => Some(&[("object-position", "right")]),
        "object-top" => Some(&[("object-position", "top")]),
        "object-top-left" => Some(&[("object-position", "top left")]),
        "object-top-right" => Some(&[("object-position", "top right")]),
        "object-bottom-left" => Some(&[("object-position", "bottom left")]),
        "object-bottom-right" => Some(&[("object-position", "bottom right")]),

        // Aspect ratio
        "aspect-auto" => Some(&[("aspect-ratio", "auto")]),
        "aspect-square" => Some(&[("aspect-ratio", "1 / 1")]),
        "aspect-video" => Some(&[("aspect-ratio", "16 / 9")]),

        // Box Sizing & Decoration
        "box-border" => Some(&[("box-sizing", "border-box")]),
        "box-content" => Some(&[("box-sizing", "content-box")]),
        "box-decoration-clone" => Some(&[("box-decoration-break", "clone")]),
        "box-decoration-slice" => Some(&[("box-decoration-break", "slice")]),

        // Clear & Float
        "clear-left" => Some(&[("clear", "left")]),
        "clear-right" => Some(&[("clear", "right")]),
        "clear-both" => Some(&[("clear", "both")]),
        "clear-none" => Some(&[("clear", "none")]),
        "clear-start" => Some(&[("clear", "inline-start")]),
        "clear-end" => Some(&[("clear", "inline-end")]),
        "float-left" => Some(&[("float", "left")]),
        "float-right" => Some(&[("float", "right")]),
        "float-none" => Some(&[("float", "none")]),
        "float-start" => Some(&[("float", "inline-start")]),
        "float-end" => Some(&[("float", "inline-end")]),

        // Isolation
        "isolate" => Some(&[("isolation", "isolate")]),
        "isolation-auto" => Some(&[("isolation", "auto")]),

        // Breaks
        "break-after-auto" => Some(&[("break-after", "auto")]),
        "break-after-avoid" => Some(&[("break-after", "avoid")]),
        "break-after-all" => Some(&[("break-after", "all")]),
        "break-after-avoid-page" => Some(&[("break-after", "avoid-page")]),
        "break-after-page" => Some(&[("break-after", "page")]),
        "break-after-left" => Some(&[("break-after", "left")]),
        "break-after-right" => Some(&[("break-after", "right")]),
        "break-after-column" => Some(&[("break-after", "column")]),
        "break-before-auto" => Some(&[("break-before", "auto")]),
        "break-before-avoid" => Some(&[("break-before", "avoid")]),
        "break-before-all" => Some(&[("break-before", "all")]),
        "break-before-avoid-page" => Some(&[("break-before", "avoid-page")]),
        "break-before-page" => Some(&[("break-before", "page")]),
        "break-before-left" => Some(&[("break-before", "left")]),
        "break-before-right" => Some(&[("break-before", "right")]),
        "break-before-column" => Some(&[("break-before", "column")]),
        "break-inside-auto" => Some(&[("break-inside", "auto")]),
        "break-inside-avoid" => Some(&[("break-inside", "avoid")]),
        "break-inside-avoid-page" => Some(&[("break-inside", "avoid-page")]),
        "break-inside-avoid-column" => Some(&[("break-inside", "avoid-column")]),
        "break-inside-avoid-flex" => Some(&[("break-inside", "avoid-flex")]),

        // Contain & Container & Content
        "contain-none" => Some(&[("contain", "none")]),
        "contain-strict" => Some(&[("contain", "strict")]),
        "contain-content" => Some(&[("contain", "content")]),
        "contain-size" => Some(&[("contain", "size")]),
        "contain-inline-size" => Some(&[("contain", "inline-size")]),
        "contain-layout" => Some(&[("contain", "layout")]),
        "contain-paint" => Some(&[("contain", "paint")]),
        "contain-style" => Some(&[("contain", "style")]),
        "container" | "@container" => Some(&[("container-type", "inline-size")]),
        "@container-normal" => Some(&[("container-type", "normal")]),
        "content-none" => Some(&[("content", "none")]),

        // Field-Sizing
        "field-sizing-content" => Some(&[("field-sizing", "content")]),
        "field-sizing-fixed" => Some(&[("field-sizing", "fixed")]),

        _ => None,
    };

    rules.map(|r| r.iter().map(|&(k, v)| (k, v.to_string())).collect())
}
