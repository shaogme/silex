use heck::AsSnakeCase;
use std::fs;
use std::path::Path;

mod css;
mod tags;

use tags::codegen::generate_module_content;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let should_fetch = args.contains(&"--fetch".to_string());

    // 1. Determine paths
    let current_dir = std::env::current_dir()?;
    let (mdn_compat_path, mdn_props_path, mdn_syntaxes_path, out_dir, css_out_dir) = if current_dir
        .join("tools/silex_codegen")
        .exists()
    {
        (
            current_dir.join("tools/silex_codegen/mdn_compat_data.json"),
            current_dir.join("tools/silex_codegen/mdn_css_properties.json"),
            current_dir.join("tools/silex_codegen/mdn_css_syntaxes.json"),
            current_dir.join("silex_html/src/tags"),
            current_dir.join("silex_css/src"),
        )
    } else if current_dir.ends_with("silex_codegen") {
        (
            current_dir.join("mdn_compat_data.json"),
            current_dir.join("mdn_css_properties.json"),
            current_dir.join("mdn_css_syntaxes.json"),
            current_dir.join("../../silex_html/src/tags"),
            current_dir.join("../../silex_css/src"),
        )
    } else {
        return Err(
                "Could not detect project root. Please run from workspace root or tools/silex_codegen directory."
                    .into(),
            );
    };

    println!("MDN Compat: {}", mdn_compat_path.display());
    println!("MDN Props:  {}", mdn_props_path.display());
    println!("MDN Syntax: {}", mdn_syntaxes_path.display());
    println!("Output dir: {}", out_dir.display());
    println!("CSS dir:    {}", css_out_dir.display());

    // 2. FETCH MODE: Raw data downloader
    if should_fetch {
        println!("\n[FETCH MODE] Fetching raw data from MDN...");

        // Simple synchronous fetch utility
        let client = reqwest::blocking::Client::builder()
            .user_agent("silex-codegen")
            .build()?;

        let fetch_and_save = |url: &str, path: &Path| -> Result<(), Box<dyn std::error::Error>> {
            println!("Downloading from {} ...", url);
            let response = client.get(url).send()?.error_for_status()?;
            let value: serde_json::Value = serde_json::from_reader(response)?;
            let file = fs::File::create(path)?;
            let writer = std::io::BufWriter::new(file);
            serde_json::to_writer_pretty(writer, &value)?;
            println!("[FETCH MODE] Saved to {}", path.display());
            Ok(())
        };

        fetch_and_save(
            "https://unpkg.com/@mdn/browser-compat-data/data.json",
            &mdn_compat_path,
        )?;
        fetch_and_save(
            "https://raw.githubusercontent.com/mdn/data/main/css/properties.json",
            &mdn_props_path,
        )?;
        fetch_and_save(
            "https://raw.githubusercontent.com/mdn/data/main/css/syntaxes.json",
            &mdn_syntaxes_path,
        )?;

        println!("\n[FETCH MODE] Download complete. Exiting.");
        return Ok(());
    }

    // 3. CODEGEN MODE: Load Source of Truth from downloaded JSON files
    if !mdn_compat_path.exists() || !mdn_props_path.exists() || !mdn_syntaxes_path.exists() {
        return Err("Missing MDN data. Please run with --fetch first.".into());
    }

    println!("\n[CODEGEN MODE] Parsing data from local MDN files...");
    let compat_str = fs::read_to_string(&mdn_compat_path)?;
    let props_str = fs::read_to_string(&mdn_props_path)?;
    let syntaxes_str = fs::read_to_string(&mdn_syntaxes_path)?;

    let config = tags::parse_tags(&compat_str)?;
    let css_config = css::parse_css(&props_str, &syntaxes_str)?;

    println!("[CODEGEN MODE] Applying in-memory patches...");
    let mut gen_config = config.clone();
    tags::apply_memory_only_patches(&mut gen_config);

    // 4. Generate and Write Rust Code
    if !out_dir.exists() {
        fs::create_dir_all(&out_dir)?;
    }
    if !css_out_dir.exists() {
        fs::create_dir_all(&css_out_dir)?;
    }

    // --- CSS Codegen ---
    let properties_code = css::generate_properties_macro(&css_config.properties);
    fs::write(css_out_dir.join("properties.rs"), properties_code)?;
    println!("Generated properties.rs");

    let keywords_code = css::generate_keywords_code(&css_config.properties);
    fs::write(css_out_dir.join("keywords_gen.rs"), keywords_code)?;
    println!("Generated keywords_gen.rs");

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
