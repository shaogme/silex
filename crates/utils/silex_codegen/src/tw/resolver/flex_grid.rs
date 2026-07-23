/// Flexbox & Grid
pub fn resolve_flex_grid_rules(class_name: &str) -> Option<Vec<(&'static str, String)>> {
    // Grid Cols
    if let Some(rest) = class_name.strip_prefix("grid-cols-") {
        if rest == "none" {
            return Some(vec![("grid-template-columns", "none".to_string())]);
        }
        if rest == "subgrid" {
            return Some(vec![("grid-template-columns", "subgrid".to_string())]);
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(vec![("grid-template-columns", format!("repeat({}, minmax(0, 1fr))", n))]);
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
            return Some(vec![("grid-template-rows", format!("repeat({}, minmax(0, 1fr))", n))]);
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

