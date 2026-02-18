use heck::{AsPascalCase, AsSnakeCase};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// --- Config Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TagDef {
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
struct TagConfig {
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let should_fetch = args.contains(&"--fetch".to_string());

    // 1. Determine paths
    let current_dir = std::env::current_dir()?;
    let (tags_path_str, out_dir_str) = if current_dir.join("tools/silex_codegen/tags.json").exists()
    {
        ("tools/silex_codegen/tags.json", "silex_html/src/tags")
    } else if current_dir.join("tags.json").exists() {
        ("tags.json", "../../silex_html/src/tags")
    } else {
        return Err(
            "Could not find tags.json. Please run from workspace root or tools/codegen directory."
                .into(),
        );
    };

    let tags_path = Path::new(tags_path_str);
    let out_dir = Path::new(out_dir_str);

    println!("Config file: {}", tags_path.display());
    println!("Output dir:  {}", out_dir.display());

    // 2. Load existing config (Source of Truth)
    let mut config = load_config(tags_path)?;

    // 3. FETCH MODE: Modify tags.json ONLY here
    if should_fetch {
        println!("\n[FETCH MODE] Fetching data from MDN...");
        fetch_and_merge_tags(&mut config)?;

        // Save the CLEAN config (without rust-specific patches) back to tags.json
        // STRICT RULE: This is the ONLY place tags.json is written to.
        let updated_json = serde_json::to_string_pretty(&config)?;
        fs::write(tags_path, updated_json)?;
        println!("[FETCH MODE] Updated {}", tags_path.display());
    } else {
        println!("\n[CODEGEN MODE] Using existing tags.json (Read-Only)");
    }

    // 4. CODEGEN MODE: In-Memory Processing
    // We clone the config to ensure the generation logic operates on a separate instance
    // that includes patches, while the file on disk remains untouched/clean.
    let mut gen_config = config.clone();

    // Apply patches for traits that are required for compilation but NOT stored in tags.json
    // STRICT RULE: These changes happen in memory only.
    apply_memory_only_patches(&mut gen_config);

    // 5. Generate and Write Rust Code
    if !out_dir.exists() {
        fs::create_dir_all(out_dir)?;
    }

    // Generate HTML module
    let html_code = generate_module_content(&gen_config.html, false, &[]);
    fs::write(out_dir.join("html.rs"), html_code)?;
    println!("Generated html.rs");

    // Collect HTML macro names to avoid collisions in SVG
    let html_macros: Vec<String> = gen_config
        .html
        .iter()
        .map(|t| {
            t.func_name
                .clone()
                .unwrap_or_else(|| AsSnakeCase(&t.struct_name).to_string())
        })
        .collect();

    // Generate SVG module
    let svg_code = generate_module_content(&gen_config.svg, true, &html_macros);
    fs::write(out_dir.join("svg.rs"), svg_code)?;
    println!("Generated svg.rs");

    println!("\nSuccessfully completed!");
    Ok(())
}

fn load_config(path: &Path) -> Result<TagConfig, Box<dyn std::error::Error>> {
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

fn fetch_and_merge_tags(config: &mut TagConfig) -> Result<(), Box<dyn std::error::Error>> {
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
        // Skip meta-properties (keys starting with __) or obsolete tags if desired.
        // For now, we accept all element keys.

        if existing_map.contains_key(&tag_name) {
            // Already exists. We DO NOT overwrite existing manual config.
            continue;
        }

        // New tag determination
        let struct_name = sanitize_struct_name(&tag_name, is_svg);
        let is_void = if is_svg {
            SVG_SHAPE_ELEMENTS.contains(&tag_name.as_str())
        } else {
            HTML_VOID_ELEMENTS.contains(&tag_name.as_str())
        };

        // Basic default traits (stored in JSON)
        // Note: Specific logic traits like FormTag are now applied in memory only.
        // We only persist the most basic structural traits here.
        let mut traits = if is_svg {
            vec!["SvgTag".to_string()]
        } else {
            vec![]
        };

        // Default heuristic: non-void elements usually contain text
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

/// Applies Rust-specific traits that are required for the library to function
/// but strictly should NOT be persisted to the JSON source of truth.
fn apply_memory_only_patches(config: &mut TagConfig) {
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

// --- Generation Logic ---

fn generate_module_content(tags: &[TagDef], is_svg: bool, forbidden_macros: &[String]) -> String {
    let mut code = String::new();
    let namespace = if is_svg { "svg" } else { "html" };
    let method_name = if is_svg { "new_svg" } else { "new" };

    // --- Tags ---
    code.push_str("// --- Tags ---\n");
    for tag in tags {
        let fn_name = tag
            .func_name
            .clone()
            .unwrap_or_else(|| AsSnakeCase(&tag.struct_name).to_string());

        let kind = if tag.is_void { "void" } else { "non_void" };
        let trait_list = tag.traits.join(", ");

        code.push_str(&format!(
            "silex_dom::define_tag!({}, \"{}\", {}, {}, {}, [{}]);\n",
            tag.struct_name, tag.tag_name, fn_name, method_name, kind, trait_list
        ));
    }

    // --- Public Macros ---
    code.push_str("\n// --- Macros ---\n");
    for tag in tags {
        let fn_name = tag
            .func_name
            .clone()
            .unwrap_or_else(|| AsSnakeCase(&tag.struct_name).to_string());

        if !tag.is_void {
            let macro_name = if forbidden_macros.contains(&fn_name) {
                format!("svg_{}", fn_name)
            } else {
                fn_name.clone()
            };

            code.push_str(&format!("#[macro_export] macro_rules! {} {{\n", macro_name));
            code.push_str(&format!(
                "    () => {{ $crate::{}::{}(()) }};\n",
                namespace, fn_name
            ));
            code.push_str(&format!(
                "    ($($child:expr),+ $(,)?) => {{ $crate::{}::{}(($($child),+)) }};\n",
                namespace, fn_name
            ));
            code.push_str("}\n");
        }
    }

    code
}

// --- Helpers ---

fn sanitize_struct_name(tag_name: &str, is_svg: bool) -> String {
    let pascal = AsPascalCase(tag_name).to_string();
    // avoid Rust keywords
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
