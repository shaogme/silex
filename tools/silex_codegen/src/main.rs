use heck::AsSnakeCase;
use std::fs;
use std::path::Path;

mod codegen;
mod tags;

use codegen::generate_module_content;
use tags::{apply_memory_only_patches, fetch_and_merge_tags, load_config};

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
