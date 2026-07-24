use std::borrow::Cow;

/// Flexbox & Grid
pub fn resolve_flex_grid_rules(class_name: &str) -> Option<Vec<(&'static str, Cow<'static, str>)>> {
    // 静态 Flexbox & Grid 规则匹配
    let static_rules: Option<&'static [(&'static str, Cow<'static, str>)]> = match class_name {
        // Flex Direction / Wrap / Flex / Grow / Shrink
        "flex-row" => Some(cow![("flex-direction", "row")]),
        "flex-row-reverse" => Some(cow![("flex-direction", "row-reverse")]),
        "flex-col" => Some(cow![("flex-direction", "column")]),
        "flex-col-reverse" => Some(cow![("flex-direction", "column-reverse")]),
        "flex-wrap" => Some(cow![("flex-wrap", "wrap")]),
        "flex-wrap-reverse" => Some(cow![("flex-wrap", "wrap-reverse")]),
        "flex-nowrap" => Some(cow![("flex-wrap", "nowrap")]),
        "flex-1" => Some(cow![("flex", "1 1 0%")]),
        "flex-auto" => Some(cow![("flex", "1 1 auto")]),
        "flex-initial" => Some(cow![("flex", "0 1 auto")]),
        "flex-none" => Some(cow![("flex", "none")]),
        "grow" => Some(cow![("flex-grow", "1")]),
        "grow-0" => Some(cow![("flex-grow", "0")]),
        "shrink" => Some(cow![("flex-shrink", "1")]),
        "shrink-0" => Some(cow![("flex-shrink", "0")]),

        // Justify Content & Items & Self
        "justify-start" => Some(cow![("justify-content", "flex-start")]),
        "justify-end" => Some(cow![("justify-content", "flex-end")]),
        "justify-center" => Some(cow![("justify-content", "center")]),
        "justify-between" => Some(cow![("justify-content", "space-between")]),
        "justify-around" => Some(cow![("justify-content", "space-around")]),
        "justify-evenly" => Some(cow![("justify-content", "space-evenly")]),
        "justify-stretch" => Some(cow![("justify-content", "stretch")]),
        "justify-normal" => Some(cow![("justify-content", "normal")]),
        "justify-baseline" => Some(cow![("justify-content", "baseline")]),
        "justify-center-safe" => Some(cow![("justify-content", "safe center")]),
        "justify-end-safe" => Some(cow![("justify-content", "safe end")]),

        "justify-items-start" => Some(cow![("justify-items", "start")]),
        "justify-items-end" => Some(cow![("justify-items", "end")]),
        "justify-items-center" => Some(cow![("justify-items", "center")]),
        "justify-items-stretch" => Some(cow![("justify-items", "stretch")]),
        "justify-items-normal" => Some(cow![("justify-items", "normal")]),
        "justify-items-center-safe" => Some(cow![("justify-items", "safe center")]),
        "justify-items-end-safe" => Some(cow![("justify-items", "safe end")]),

        "justify-self-auto" => Some(cow![("justify-self", "auto")]),
        "justify-self-start" => Some(cow![("justify-self", "start")]),
        "justify-self-end" => Some(cow![("justify-self", "end")]),
        "justify-self-center" => Some(cow![("justify-self", "center")]),
        "justify-self-stretch" => Some(cow![("justify-self", "stretch")]),
        "justify-self-center-safe" => Some(cow![("justify-self", "safe center")]),
        "justify-self-end-safe" => Some(cow![("justify-self", "safe end")]),

        // Align Items & Align Self
        "items-start" => Some(cow![("align-items", "flex-start")]),
        "items-end" => Some(cow![("align-items", "flex-end")]),
        "items-center" => Some(cow![("align-items", "center")]),
        "items-baseline" => Some(cow![("align-items", "baseline")]),
        "items-stretch" => Some(cow![("align-items", "stretch")]),
        "items-baseline-last" => Some(cow![("align-items", "last baseline")]),
        "items-center-safe" => Some(cow![("align-items", "safe center")]),
        "items-end-safe" => Some(cow![("align-items", "safe end")]),

        "self-auto" => Some(cow![("align-self", "auto")]),
        "self-start" => Some(cow![("align-self", "flex-start")]),
        "self-end" => Some(cow![("align-self", "flex-end")]),
        "self-center" => Some(cow![("align-self", "center")]),
        "self-stretch" => Some(cow![("align-self", "stretch")]),
        "self-baseline" => Some(cow![("align-self", "baseline")]),
        "self-baseline-last" => Some(cow![("align-self", "last baseline")]),
        "self-center-safe" => Some(cow![("align-self", "safe center")]),
        "self-end-safe" => Some(cow![("align-self", "safe end")]),

        // Align Content
        "content-normal" => Some(cow![("align-content", "normal")]),
        "content-center" => Some(cow![("align-content", "center")]),
        "content-start" => Some(cow![("align-content", "flex-start")]),
        "content-end" => Some(cow![("align-content", "flex-end")]),
        "content-between" => Some(cow![("align-content", "space-between")]),
        "content-around" => Some(cow![("align-content", "space-around")]),
        "content-evenly" => Some(cow![("align-content", "space-evenly")]),
        "content-baseline" => Some(cow![("align-content", "baseline")]),
        "content-stretch" => Some(cow![("align-content", "stretch")]),
        "content-center-safe" => Some(cow![("align-content", "safe center")]),
        "content-end-safe" => Some(cow![("align-content", "safe end")]),

        // Place Extensions Safe
        "place-content-center-safe" => Some(cow![("place-content", "safe center")]),
        "place-content-end-safe" => Some(cow![("place-content", "safe end")]),
        "place-items-center-safe" => Some(cow![("place-items", "safe center")]),
        "place-items-end-safe" => Some(cow![("place-items", "safe end")]),
        "place-self-center-safe" => Some(cow![("place-self", "safe center")]),
        "place-self-end-safe" => Some(cow![("place-self", "safe end")]),

        _ => None,
    };

    if let Some(r) = static_rules {
        return Some(r.to_vec());
    }

    // Grid Cols
    if let Some(rest) = class_name.strip_prefix("grid-cols-") {
        if rest == "none" {
            return Some(cow!(vec[("grid-template-columns", "none")]));
        }
        if rest == "subgrid" {
            return Some(cow!(vec[("grid-template-columns", "subgrid")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(
                vec[(
                    "grid-template-columns",
                    format!("repeat({}, minmax(0, 1fr))", n),
                )]
            ));
        }
    }

    // Grid Rows
    if let Some(rest) = class_name.strip_prefix("grid-rows-") {
        if rest == "none" {
            return Some(cow!(vec[("grid-template-rows", "none")]));
        }
        if rest == "subgrid" {
            return Some(cow!(vec[("grid-template-rows", "subgrid")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(
                vec[(
                    "grid-template-rows",
                    format!("repeat({}, minmax(0, 1fr))", n),
                )]
            ));
        }
    }

    if class_name == "col-auto" {
        return Some(cow!(vec[("grid-column", "auto")]));
    }
    if class_name == "row-auto" {
        return Some(cow!(vec[("grid-row", "auto")]));
    }

    // Col Span / Start / End
    if let Some(rest) = class_name.strip_prefix("col-span-") {
        if rest == "full" {
            return Some(cow!(vec[("grid-column", "1 / -1")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(
                vec[("grid-column", format!("span {} / span {}", n, n))]
            ));
        }
    }
    if let Some(rest) = class_name.strip_prefix("col-start-") {
        if rest == "auto" {
            return Some(cow!(vec[("grid-column-start", "auto")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(vec[("grid-column-start", n.to_string())]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("-col-start-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("grid-column-start", format!("-{}", n))]));
    }
    if let Some(rest) = class_name.strip_prefix("col-end-") {
        if rest == "auto" {
            return Some(cow!(vec[("grid-column-end", "auto")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(vec[("grid-column-end", n.to_string())]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("-col-end-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("grid-column-end", format!("-{}", n))]));
    }

    // Row Span / Start / End
    if let Some(rest) = class_name.strip_prefix("row-span-") {
        if rest == "full" {
            return Some(cow!(vec[("grid-row", "1 / -1")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(vec[("grid-row", format!("span {} / span {}", n, n))]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("row-start-") {
        if rest == "auto" {
            return Some(cow!(vec[("grid-row-start", "auto")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(vec[("grid-row-start", n.to_string())]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("-row-start-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("grid-row-start", format!("-{}", n))]));
    }
    if let Some(rest) = class_name.strip_prefix("row-end-") {
        if rest == "auto" {
            return Some(cow!(vec[("grid-row-end", "auto")]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(vec[("grid-row-end", n.to_string())]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("-row-end-")
        && let Ok(n) = rest.parse::<u32>()
    {
        return Some(cow!(vec[("grid-row-end", format!("-{}", n))]));
    }

    // Flex fractions & numbers (flex-1/2, flex-2, etc.)
    if let Some(rest) = class_name.strip_prefix("flex-") {
        if let Some(val) = super::dynamic::resolve_length_val(rest) {
            return Some(cow!(vec[("flex", format!("1 1 {}", val))]));
        }
        if let Ok(n) = rest.parse::<u32>() {
            return Some(cow!(vec[("flex", format!("{} {} 0%", n, n))]));
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
            return Some(cow!(vec[("order", ord.to_string())]));
        }
    }
    if let Some(rest) = class_name.strip_prefix("-order-")
        && let Ok(n) = rest.parse::<i32>()
    {
        return Some(cow!(vec[("order", format!("-{}", n))]));
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
            return Some(cow!(vec[("grid-auto-columns", v)]));
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
            return Some(cow!(vec[("grid-auto-rows", v)]));
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
            return Some(cow!(vec[("grid-auto-flow", v)]));
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
            return Some(cow!(vec[("place-content", v)]));
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
            return Some(cow!(vec[("place-items", v)]));
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
            return Some(cow!(vec[("place-self", v)]));
        }
    }

    None
}
