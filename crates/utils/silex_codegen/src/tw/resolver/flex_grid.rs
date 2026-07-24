/// Flexbox & Grid
pub fn resolve_flex_grid_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    // 静态 Flexbox & Grid 规则匹配
    let static_rules: Option<&'static [(&'static str, &'static str)]> = match class_name {
        // Flex Direction / Wrap / Flex / Grow / Shrink
        "flex-row" => Some(&[("flex-direction", "row")]),
        "flex-row-reverse" => Some(&[("flex-direction", "row-reverse")]),
        "flex-col" => Some(&[("flex-direction", "column")]),
        "flex-col-reverse" => Some(&[("flex-direction", "column-reverse")]),
        "flex-wrap" => Some(&[("flex-wrap", "wrap")]),
        "flex-wrap-reverse" => Some(&[("flex-wrap", "wrap-reverse")]),
        "flex-nowrap" => Some(&[("flex-wrap", "nowrap")]),
        "flex-1" => Some(&[("flex", "1 1 0%")]),
        "flex-auto" => Some(&[("flex", "1 1 auto")]),
        "flex-initial" => Some(&[("flex", "0 1 auto")]),
        "flex-none" => Some(&[("flex", "none")]),
        "grow" => Some(&[("flex-grow", "1")]),
        "grow-0" => Some(&[("flex-grow", "0")]),
        "shrink" => Some(&[("flex-shrink", "1")]),
        "shrink-0" => Some(&[("flex-shrink", "0")]),

        // Justify Content & Items & Self
        "justify-start" => Some(&[("justify-content", "flex-start")]),
        "justify-end" => Some(&[("justify-content", "flex-end")]),
        "justify-center" => Some(&[("justify-content", "center")]),
        "justify-between" => Some(&[("justify-content", "space-between")]),
        "justify-around" => Some(&[("justify-content", "space-around")]),
        "justify-evenly" => Some(&[("justify-content", "space-evenly")]),
        "justify-stretch" => Some(&[("justify-content", "stretch")]),
        "justify-normal" => Some(&[("justify-content", "normal")]),
        "justify-baseline" => Some(&[("justify-content", "baseline")]),
        "justify-center-safe" => Some(&[("justify-content", "safe center")]),
        "justify-end-safe" => Some(&[("justify-content", "safe end")]),

        "justify-items-start" => Some(&[("justify-items", "start")]),
        "justify-items-end" => Some(&[("justify-items", "end")]),
        "justify-items-center" => Some(&[("justify-items", "center")]),
        "justify-items-stretch" => Some(&[("justify-items", "stretch")]),
        "justify-items-normal" => Some(&[("justify-items", "normal")]),
        "justify-items-center-safe" => Some(&[("justify-items", "safe center")]),
        "justify-items-end-safe" => Some(&[("justify-items", "safe end")]),

        "justify-self-auto" => Some(&[("justify-self", "auto")]),
        "justify-self-start" => Some(&[("justify-self", "start")]),
        "justify-self-end" => Some(&[("justify-self", "end")]),
        "justify-self-center" => Some(&[("justify-self", "center")]),
        "justify-self-stretch" => Some(&[("justify-self", "stretch")]),
        "justify-self-center-safe" => Some(&[("justify-self", "safe center")]),
        "justify-self-end-safe" => Some(&[("justify-self", "safe end")]),

        // Align Items & Align Self
        "items-start" => Some(&[("align-items", "flex-start")]),
        "items-end" => Some(&[("align-items", "flex-end")]),
        "items-center" => Some(&[("align-items", "center")]),
        "items-baseline" => Some(&[("align-items", "baseline")]),
        "items-stretch" => Some(&[("align-items", "stretch")]),
        "items-baseline-last" => Some(&[("align-items", "last baseline")]),
        "items-center-safe" => Some(&[("align-items", "safe center")]),
        "items-end-safe" => Some(&[("align-items", "safe end")]),

        "self-auto" => Some(&[("align-self", "auto")]),
        "self-start" => Some(&[("align-self", "flex-start")]),
        "self-end" => Some(&[("align-self", "flex-end")]),
        "self-center" => Some(&[("align-self", "center")]),
        "self-stretch" => Some(&[("align-self", "stretch")]),
        "self-baseline" => Some(&[("align-self", "baseline")]),
        "self-baseline-last" => Some(&[("align-self", "last baseline")]),
        "self-center-safe" => Some(&[("align-self", "safe center")]),
        "self-end-safe" => Some(&[("align-self", "safe end")]),

        // Align Content
        "content-normal" => Some(&[("align-content", "normal")]),
        "content-center" => Some(&[("align-content", "center")]),
        "content-start" => Some(&[("align-content", "flex-start")]),
        "content-end" => Some(&[("align-content", "flex-end")]),
        "content-between" => Some(&[("align-content", "space-between")]),
        "content-around" => Some(&[("align-content", "space-around")]),
        "content-evenly" => Some(&[("align-content", "space-evenly")]),
        "content-baseline" => Some(&[("align-content", "baseline")]),
        "content-stretch" => Some(&[("align-content", "stretch")]),
        "content-center-safe" => Some(&[("align-content", "safe center")]),
        "content-end-safe" => Some(&[("align-content", "safe end")]),

        // Place Extensions Safe
        "place-content-center-safe" => Some(&[("place-content", "safe center")]),
        "place-content-end-safe" => Some(&[("place-content", "safe end")]),
        "place-items-center-safe" => Some(&[("place-items", "safe center")]),
        "place-items-end-safe" => Some(&[("place-items", "safe end")]),
        "place-self-center-safe" => Some(&[("place-self", "safe center")]),
        "place-self-end-safe" => Some(&[("place-self", "safe end")]),

        _ => None,
    };

    if let Some(r) = static_rules {
        return Some(r.iter().map(|&(k, v)| (k, v.to_string())).collect());
    }

    // Grid Cols
    if let Some(rest) = class_name.strip_prefix("grid-cols-") {
        if rest == "none" {
            return Some(vec![("grid-template-columns", "none".to_string())]);
        }
        if rest == "subgrid" {
            return Some(vec![("grid-template-columns", "subgrid".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(
                "grid-template-columns",
                format!("repeat({}, minmax(0, 1fr))", n),
            )]);
        }
    }

    // Grid Rows
    if let Some(rest) = class_name.strip_prefix("grid-rows-") {
        if rest == "none" {
            return Some(vec![("grid-template-rows", "none".to_string())]);
        }
        if rest == "subgrid" {
            return Some(vec![("grid-template-rows", "subgrid".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![(
                "grid-template-rows",
                format!("repeat({}, minmax(0, 1fr))", n),
            )]);
        }
    }

    if class_name == "col-auto" {
        return Some(vec![("grid-column", "auto".to_string())]);
    }
    if class_name == "row-auto" {
        return Some(vec![("grid-row", "auto".to_string())]);
    }

    // Col Span / Start / End
    if let Some(rest) = class_name.strip_prefix("col-span-") {
        if rest == "full" {
            return Some(vec![("grid-column", "1 / -1".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-column", format!("span {} / span {}", n, n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("col-start-") {
        if rest == "auto" {
            return Some(vec![("grid-column-start", "auto".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-column-start", n.to_string())]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-col-start-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-column-start", format!("-{}", n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("col-end-") {
        if rest == "auto" {
            return Some(vec![("grid-column-end", "auto".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-column-end", n.to_string())]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-col-end-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-column-end", format!("-{}", n))]);
        }
    }

    // Row Span / Start / End
    if let Some(rest) = class_name.strip_prefix("row-span-") {
        if rest == "full" {
            return Some(vec![("grid-row", "1 / -1".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-row", format!("span {} / span {}", n, n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("row-start-") {
        if rest == "auto" {
            return Some(vec![("grid-row-start", "auto".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-row-start", n.to_string())]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-row-start-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-row-start", format!("-{}", n))]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("row-end-") {
        if rest == "auto" {
            return Some(vec![("grid-row-end", "auto".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-row-end", n.to_string())]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-row-end-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-row-end", format!("-{}", n))]);
        }
    }

    // Flex fractions & numbers (flex-1/2, flex-2, etc.)
    if let Some(rest) = class_name.strip_prefix("flex-") {
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(vec![("flex", format!("1 1 {}", val))]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("flex", format!("{} {} 0%", n, n))]);
        }
    }

    // Order
    if let Some(rest) = class_name.strip_prefix("order-") {
        let ord = match rest {
            "first" => "-9999",
            "last" => "9999",
            "none" => "0",
            _ => rest,
        };
        if ord.parse::<i32>().is_ok() {
            return Some(vec![("order", ord.to_string())]);
        }
    }
    if let Some(rest) = class_name.strip_prefix("-order-") {
        if let Ok(n) = rest.parse::<i32>() {
            return Some(vec![("order", format!("-{}", n))]);
        }
    }

    // Grid Auto Cols
    if let Some(rest) = class_name.strip_prefix("auto-cols-") {
        let val = match rest {
            "auto" => Some("auto"),
            "min" => Some("min-content"),
            "max" => Some("max-content"),
            "fr" => Some("minmax(0, 1fr)"),
            _ => None,
        };
        if let Some(v) = val {
            return Some(vec![("grid-auto-columns", v.to_string())]);
        }
    }

    // Grid Auto Rows
    if let Some(rest) = class_name.strip_prefix("auto-rows-") {
        let val = match rest {
            "auto" => Some("auto"),
            "min" => Some("min-content"),
            "max" => Some("max-content"),
            "fr" => Some("minmax(0, 1fr)"),
            _ => None,
        };
        if let Some(v) = val {
            return Some(vec![("grid-auto-rows", v.to_string())]);
        }
    }

    // Grid Auto Flow
    if let Some(rest) = class_name.strip_prefix("grid-flow-") {
        let val = match rest {
            "row" => Some("row"),
            "col" => Some("column"),
            "dense" => Some("dense"),
            "row-dense" => Some("row dense"),
            "col-dense" => Some("column dense"),
            _ => None,
        };
        if let Some(v) = val {
            return Some(vec![("grid-auto-flow", v.to_string())]);
        }
    }

    // Place Content
    if let Some(rest) = class_name.strip_prefix("place-content-") {
        let val = match rest {
            "center" => Some("center"),
            "start" => Some("start"),
            "end" => Some("end"),
            "between" => Some("space-between"),
            "around" => Some("space-around"),
            "evenly" => Some("space-evenly"),
            "baseline" => Some("baseline"),
            "stretch" => Some("stretch"),
            _ => None,
        };
        if let Some(v) = val {
            return Some(vec![("place-content", v.to_string())]);
        }
    }

    // Place Items
    if let Some(rest) = class_name.strip_prefix("place-items-") {
        let val = match rest {
            "start" => Some("start"),
            "end" => Some("end"),
            "center" => Some("center"),
            "baseline" => Some("baseline"),
            "stretch" => Some("stretch"),
            _ => None,
        };
        if let Some(v) = val {
            return Some(vec![("place-items", v.to_string())]);
        }
    }

    // Place Self
    if let Some(rest) = class_name.strip_prefix("place-self-") {
        let val = match rest {
            "auto" => Some("auto"),
            "start" => Some("start"),
            "end" => Some("end"),
            "center" => Some("center"),
            "stretch" => Some("stretch"),
            _ => None,
        };
        if let Some(v) = val {
            return Some(vec![("place-self", v.to_string())]);
        }
    }

    None
}
