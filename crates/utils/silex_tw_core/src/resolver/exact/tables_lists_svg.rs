use std::borrow::Cow;

/// SVG、列表与表格静态规则解析
pub fn resolve_tables_lists_svg_rules(
    class_name: &str,
) -> Option<&'static [(&'static str, Cow<'static, str>)]> {
    match class_name {
        // SVG
        "fill-none" => Some(cow![("fill", "none")]),
        "stroke-none" => Some(cow![("stroke", "none")]),
        "stroke-0" => Some(cow![("stroke-width", "0")]),
        "stroke-1" => Some(cow![("stroke-width", "1px")]),
        "stroke-2" => Some(cow![("stroke-width", "2px")]),
        "stroke-3" => Some(cow![("stroke-width", "3px")]),

        // List & Table
        "list-inside" => Some(cow![("list-style-position", "inside")]),
        "list-outside" => Some(cow![("list-style-position", "outside")]),
        "list-disc" => Some(cow![("list-style-type", "disc")]),
        "list-decimal" => Some(cow![("list-style-type", "decimal")]),
        "list-none" => Some(cow![("list-style-type", "none")]),
        "list-item" => Some(cow![("display", "list-item")]),
        "list-image-none" => Some(cow![("list-style-image", "none")]),
        "table-auto" => Some(cow![("table-layout", "auto")]),
        "table-fixed" => Some(cow![("table-layout", "fixed")]),
        "border-collapse" => Some(cow![("border-collapse", "collapse")]),
        "border-separate" => Some(cow![("border-collapse", "separate")]),
        "caption-top" => Some(cow![("caption-side", "top")]),
        "caption-bottom" => Some(cow![("caption-side", "bottom")]),
        "table-caption" => Some(cow![("display", "table-caption")]),
        "table-column" => Some(cow![("display", "table-column")]),
        "table-column-group" => Some(cow![("display", "table-column-group")]),
        "table-footer-group" => Some(cow![("display", "table-footer-group")]),
        "table-header-group" => Some(cow![("display", "table-header-group")]),
        "table-row-group" => Some(cow![("display", "table-row-group")]),

        _ => None,
    }
}
