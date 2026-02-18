use heck::AsPascalCase;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// --- Config Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagDef {
    pub struct_name: String,
    pub tag_name: String,
    // Optional function name override, defaults to snake_case of struct_name
    pub func_name: Option<String>,
    pub is_void: bool,
    // List of trait names to implement (e.g. "GlobalAttributes", "FormTag")
    #[serde(default)]
    pub traits: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagConfig {
    #[serde(default)]
    pub html: Vec<TagDef>,
    #[serde(default)]
    pub svg: Vec<TagDef>,
}

// --- MDN Data Structures ---

#[derive(Debug, Deserialize)]
struct MdnCompatData {
    pub html: Option<MdnCategory>,
    pub svg: Option<MdnCategory>,
}

#[derive(Debug, Deserialize)]
struct MdnCategory {
    pub elements: Option<HashMap<String, Value>>,
}

// --- Constants ---

const HTML_VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const SVG_SHAPE_ELEMENTS: &[&str] = &[
    "circle", "ellipse", "line", "path", "polygon", "polyline", "rect", "use", "image", "stop",
];

pub fn load_config(path: &Path) -> Result<TagConfig, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(TagConfig {
            html: vec![],
            svg: vec![],
        });
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(TagConfig {
            html: vec![],
            svg: vec![],
        });
    }

    Ok(serde_json::from_str(&content)?)
}

// --- Fetch Logic ---

pub fn fetch_and_merge_tags(config: &mut TagConfig) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder().user_agent("silex-codegen").build()?;
    let url = "https://unpkg.com/@mdn/browser-compat-data/data.json";

    println!("Downloading from {}", url);
    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch MDN data: {}", resp.status()).into());
    }

    let data: MdnCompatData = resp.json()?;

    if let Some(category) = data.html {
        if let Some(elements) = category.elements {
            merge_tag_list(&mut config.html, elements, false);
        }
    }

    if let Some(category) = data.svg {
        if let Some(elements) = category.elements {
            merge_tag_list(&mut config.svg, elements, true);
        }
    }

    Ok(())
}

fn merge_tag_list(
    existing_tags: &mut Vec<TagDef>,
    mdn_elements: HashMap<String, Value>,
    is_svg: bool,
) {
    let mut existing_map: HashMap<String, usize> = existing_tags
        .iter()
        .enumerate()
        .map(|(i, t)| (t.tag_name.clone(), i))
        .collect();

    let mut sorted_mdn_keys: Vec<String> = mdn_elements.keys().cloned().collect();
    sorted_mdn_keys.sort();

    for tag_name in sorted_mdn_keys {
        if existing_map.contains_key(&tag_name) {
            continue;
        }

        let struct_name = sanitize_struct_name(&tag_name, is_svg);
        let is_void = if is_svg {
            SVG_SHAPE_ELEMENTS.contains(&tag_name.as_str())
        } else {
            HTML_VOID_ELEMENTS.contains(&tag_name.as_str())
        };

        let mut traits = if is_svg {
            vec!["SvgTag".to_string()]
        } else {
            vec![]
        };

        if !is_void {
            traits.push("TextTag".to_string());
        }

        let new_def = TagDef {
            struct_name,
            tag_name: tag_name.clone(),
            func_name: sanitize_func_name(&tag_name),
            is_void,
            traits,
        };
        existing_tags.push(new_def);
        existing_map.insert(tag_name, existing_tags.len() - 1);
    }
}

// --- In-Memory Patch Logic ---

pub fn apply_memory_only_patches(config: &mut TagConfig) {
    for tag in &mut config.html {
        let name = tag.tag_name.clone();

        match name.as_str() {
            "input" | "textarea" | "select" | "option" | "optgroup" | "button" | "fieldset"
            | "output" | "form" => {
                ensure_trait_in_memory(tag, "FormTag");
            }
            "label" => ensure_trait_in_memory(tag, "LabelTag"),
            "a" | "area" | "link" => ensure_trait_in_memory(tag, "AnchorTag"),
            "img" | "video" | "audio" | "source" | "track" | "embed" | "iframe" | "object" => {
                ensure_trait_in_memory(tag, "MediaTag")
            }
            "details" | "dialog" => ensure_trait_in_memory(tag, "OpenTag"),
            "td" | "th" => ensure_trait_in_memory(tag, "TableCellTag"),
            _ => {}
        }

        if name == "th" {
            ensure_trait_in_memory(tag, "TableHeaderTag");
        }
    }
}

fn ensure_trait_in_memory(tag: &mut TagDef, trait_name: &str) {
    if !tag.traits.iter().any(|t| t == trait_name) {
        tag.traits.push(trait_name.to_string());
    }
}

// --- Helpers ---

fn sanitize_struct_name(tag_name: &str, is_svg: bool) -> String {
    let pascal = AsPascalCase(tag_name).to_string();
    let name = match pascal.as_str() {
        "Type" => "TypeEl".to_string(), // type is a keyword
        "Box" => "BoxEl".to_string(),   // box is a keyword
        "Loop" => "LoopEl".to_string(), // loop is a keyword
        "If" => "IfEl".to_string(),
        "For" => "ForEl".to_string(),
        "While" => "WhileEl".to_string(),
        "Mod" => "ModEl".to_string(),
        "Use" => "UseEl".to_string(),
        "Impl" => "ImplEl".to_string(),
        "Trait" => "TraitEl".to_string(),
        "Pub" => "PubEl".to_string(),
        "Struct" => "StructEl".to_string(),
        "Enum" => "EnumEl".to_string(),
        "Fn" => "FnEl".to_string(),
        "Let" => "LetEl".to_string(),
        "Mut" => "MutEl".to_string(),
        "Ref" => "RefEl".to_string(),
        "NoScript" => "NoScript".to_string(), // noscript
        _ => pascal,
    };

    if is_svg && name == "A" {
        return "SvgA".to_string(); // conflict with HTML A
    }
    if is_svg && name == "Script" {
        return "SvgScript".to_string();
    }
    if is_svg && name == "Style" {
        return "SvgStyle".to_string();
    }
    if is_svg && name == "Title" {
        return "SvgTitle".to_string();
    }

    name
}

fn sanitize_func_name(tag_name: &str) -> Option<String> {
    match tag_name {
        "type" => Some("type_el".to_string()),
        "box" => Some("box_el".to_string()),
        "loop" => Some("loop_el".to_string()),
        "if" => Some("if_el".to_string()),
        "for" => Some("for_el".to_string()),
        "while" => Some("while_el".to_string()),
        "mod" => Some("mod_el".to_string()),
        "use" => Some("use_el".to_string()),
        _ => None,
    }
}
