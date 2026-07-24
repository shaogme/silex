/// SVG、列表与表格静态规则解析
pub fn resolve_tables_lists_svg_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    let rules: Option<&'static [(&'static str, &'static str)]> = match class_name {
        // SVG
        "fill-none" => Some(&[("fill", "none")]),
        "stroke-none" => Some(&[("stroke", "none")]),
        "stroke-0" => Some(&[("stroke-width", "0")]),
        "stroke-1" => Some(&[("stroke-width", "1px")]),
        "stroke-2" => Some(&[("stroke-width", "2px")]),
        "stroke-3" => Some(&[("stroke-width", "3px")]),

        // List & Table
        "list-inside" => Some(&[("list-style-position", "inside")]),
        "list-outside" => Some(&[("list-style-position", "outside")]),
        "list-disc" => Some(&[("list-style-type", "disc")]),
        "list-decimal" => Some(&[("list-style-type", "decimal")]),
        "list-none" => Some(&[("list-style-type", "none")]),
        "list-item" => Some(&[("display", "list-item")]),
        "list-image-none" => Some(&[("list-style-image", "none")]),
        "table-auto" => Some(&[("table-layout", "auto")]),
        "table-fixed" => Some(&[("table-layout", "fixed")]),
        "border-collapse" => Some(&[("border-collapse", "collapse")]),
        "border-separate" => Some(&[("border-collapse", "separate")]),
        "caption-top" => Some(&[("caption-side", "top")]),
        "caption-bottom" => Some(&[("caption-side", "bottom")]),
        "table-caption" => Some(&[("display", "table-caption")]),
        "table-column" => Some(&[("display", "table-column")]),
        "table-column-group" => Some(&[("display", "table-column-group")]),
        "table-footer-group" => Some(&[("display", "table-footer-group")]),
        "table-header-group" => Some(&[("display", "table-header-group")]),
        "table-row-group" => Some(&[("display", "table-row-group")]),

        _ => None,
    };

    rules.map(|r| r.iter().map(|&(k, v)| (k, v.to_string())).collect())
}
