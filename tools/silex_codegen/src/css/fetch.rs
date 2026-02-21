use super::types::{CssConfig, MdnCssProperty, ProcessedProp, PropGroup};
use heck::{AsPascalCase, AsSnakeCase};
use std::collections::HashMap;

pub fn fetch_and_merge_css(config: &mut CssConfig) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("silex-codegen")
        .build()?;
    let url = "https://raw.githubusercontent.com/mdn/data/main/css/properties.json";

    println!("Downloading CSS properties from {}", url);
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch MDN CSS data: {}", resp.status()).into());
    }

    let raw_props: HashMap<String, MdnCssProperty> = resp.json()?;

    let mut existing_names: std::collections::HashSet<String> =
        config.properties.iter().map(|p| p.name.clone()).collect();

    for (name, prop) in raw_props {
        // 1. Only standard properties
        if prop.status != "standard" {
            continue;
        }

        if existing_names.contains(&name) {
            continue;
        }

        let method_name = sanitize_method_name(&name);
        let struct_name = AsPascalCase(&name).to_string();

        // Skip if empty or invalid identifiers (e.g. "--*")
        if method_name.is_empty() || struct_name.is_empty() || !is_valid_identifier(&method_name) {
            continue;
        }

        // 2. Determine Group
        let (group, keywords) = classify_property(&name, &prop);

        config.properties.push(ProcessedProp {
            name: name.clone(),
            method_name,
            struct_name,
            group,
            keywords,
        });
        existing_names.insert(name);
    }

    // Sort by name for deterministic output
    config.properties.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(())
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    for c in chars {
        if !c.is_alphanumeric() && c != '_' {
            return false;
        }
    }
    true
}

fn classify_property(name: &str, prop: &MdnCssProperty) -> (PropGroup, Vec<String>) {
    let syntax = &prop.syntax;

    // 1. Determine Keywords first (Manual Overrides for common properties)
    let manual_keywords = match name {
        "display" => Some(vec![
            "block",
            "inline",
            "inline-block",
            "flex",
            "inline-flex",
            "grid",
            "inline-grid",
            "none",
            "contents",
            "table",
            "list-item",
            "inherit",
            "initial",
            "unset",
        ]),
        "border-style"
        | "border-top-style"
        | "border-right-style"
        | "border-bottom-style"
        | "border-left-style"
        | "outline-style" => Some(vec![
            "none", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset",
            "outset",
        ]),
        "cursor" => Some(vec![
            "auto",
            "default",
            "pointer",
            "wait",
            "text",
            "move",
            "help",
            "not-allowed",
            "grab",
            "grabbing",
        ]),
        "text-decoration" => Some(vec![
            "none",
            "underline",
            "overline",
            "line-through",
            "blink",
        ]),
        "align-items" | "align-self" | "align-content" | "justify-items" | "justify-self"
        | "justify-content" => Some(vec![
            "normal",
            "stretch",
            "center",
            "flex-start",
            "flex-end",
            "start",
            "end",
            "self-start",
            "self-end",
            "space-between",
            "space-around",
            "space-evenly",
            "baseline",
        ]),
        "text-align" => Some(vec!["left", "right", "center", "justify", "start", "end"]),
        "position" => Some(vec!["static", "relative", "absolute", "fixed", "sticky"]),
        "overflow" | "overflow-x" | "overflow-y" => {
            Some(vec!["visible", "hidden", "clip", "scroll", "auto"])
        }
        "flex-direction" => Some(vec!["row", "row-reverse", "column", "column-reverse"]),
        "flex-wrap" => Some(vec!["nowrap", "wrap", "wrap-reverse"]),
        "font-weight" => Some(vec!["normal", "bold", "bolder", "lighter"]),
        "visibility" => Some(vec!["visible", "hidden", "collapse"]),
        "pointer-events" => Some(vec!["auto", "none", "inherit", "initial"]),
        _ => None,
    };

    let keywords = manual_keywords
        .map(|v| v.into_iter().map(String::from).collect())
        .unwrap_or_else(|| extract_keywords(syntax));

    // 2. Determine Group (Priority: Shorthand > Dimension > Color > Number > Keyword)
    let shorthands = [
        "margin",
        "padding",
        "border",
        "background",
        "font",
        "transition",
        "transform",
        "flex",
        "grid",
        "outline",
        "list-style",
        "columns",
        "gap",
        "box-shadow",
        "text-shadow",
        "filter",
        "backdrop-filter",
    ];
    if shorthands.contains(&name) {
        return (PropGroup::Shorthand, vec![]);
    }

    // Manual Group Overrides to match user preferences
    match name {
        "background-size" | "grid-auto-columns" | "grid-auto-rows" => {
            return (PropGroup::Custom, keywords);
        }
        "mask-clip" => return (PropGroup::Keyword, keywords),
        "mask-border-slice" => return (PropGroup::Number, keywords),
        "grid-template" => return (PropGroup::Keyword, keywords),
        _ => {}
    }

    let group = if syntax.contains("<length")
        || syntax.contains("<percentage")
        || syntax.contains("width>")
        || syntax.contains("height>")
        || syntax.contains("radius>")
        || syntax.contains("gap>")
        || syntax.contains("padding>")
        || syntax.contains("margin>")
        || syntax.contains("offset>")
        || syntax.contains("indent>")
        || syntax.contains("spacing>")
        || syntax.contains("position>")
        || name.contains("radius")
        || name.contains("width") && !name.contains("stroke")
        || name.contains("height")
        || name == "zoom"
    {
        PropGroup::Dimension
    } else if syntax.contains("<color") || syntax.contains("color>") {
        PropGroup::Color
    } else if syntax.contains("<number") || syntax.contains("<integer") || name == "font-weight" {
        PropGroup::Number
    } else if !keywords.is_empty() && keywords.len() < 30 {
        PropGroup::Keyword
    } else {
        PropGroup::Custom
    };

    (group, keywords)
}

fn extract_keywords(syntax: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    // Clean up syntax string to make splitting easier
    let syntax_clean = syntax.replace(['[', ']', '{', '}', '*', '+', '?', '#', ','], " ");

    for part in syntax_clean.split('|') {
        for subpart in part.split_whitespace() {
            let trimmed = subpart.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check if it's a plain keyword (no < > and starts with a letter)
            if !trimmed.contains('<')
                && !trimmed.contains('>')
                && trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_alphabetic())
                    .unwrap_or(false)
                && trimmed.chars().all(|c| c.is_alphanumeric() || c == '-')
            {
                parts.push(trimmed.to_string());
            }
        }
    }

    parts.sort();
    parts.dedup();
    parts
}

fn sanitize_method_name(name: &str) -> String {
    let s = AsSnakeCase(name).to_string();
    match s.as_str() {
        "type" => "type_".to_string(),
        _ => s,
    }
}
