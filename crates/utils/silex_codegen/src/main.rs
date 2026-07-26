mod css;
mod tags;
mod tw;

use crate::{
    css::{
        generate_keywords_code, generate_properties_macro, generate_property_names_code, parse_css,
    },
    tags::{apply_memory_only_patches, codegen::generate_module_content, parse_tags},
    tw::{
        CodegenBaseline, ReferenceCssJson, check_drift, fingerprint_tw_datasets,
        generate_keyframes_code, generate_macro_tables, generate_modifiers_code,
        generate_palette_code, generate_prefix_metadata_code, generate_property_id_code,
        generate_reference_css_code, generate_table_examples, validate_prefix_metadata,
        validate_resolver_properties, validate_resolver_values,
    },
};
use heck::AsSnakeCase;
use reqwest::blocking::Client;
use serde_json::{Value, from_reader, from_str, to_writer_pretty};
use std::{
    collections::BTreeMap,
    env::{args, current_dir},
    error::Error,
    fs::{File, create_dir_all, read_to_string, write},
    io::BufWriter,
    path::Path,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = args().collect();
    let should_fetch = args.contains(&"--fetch".to_string());
    // 大规模上游数据漂移需要显式确认一次，避免 Tailwind 升版本导致静默的批量语义变更
    let accept_drift = args.contains(&"--accept-drift".to_string());

    // 1. Determine paths
    let current_dir = current_dir()?;
    let (
        mdn_compat_path,
        mdn_props_path,
        mdn_syntaxes_path,
        out_dir,
        css_out_dir,
        macro_codegen_dir,
        macro_css_dir,
    ) = if current_dir.join("crates/utils/silex_codegen").exists() {
        (
            current_dir.join("crates/utils/silex_codegen/data/mdn_compat_data.json"),
            current_dir.join("crates/utils/silex_codegen/data/mdn_css_properties.json"),
            current_dir.join("crates/utils/silex_codegen/data/mdn_css_syntaxes.json"),
            current_dir.join("crates/silex_html/src/tags"),
            current_dir.join("crates/silex_css/src/codegen"),
            current_dir.join("crates/silex_macros/src/css/tw/resolver/codegen"),
            current_dir.join("crates/silex_macros/src/css"),
        )
    } else if current_dir.ends_with("silex_codegen") {
        (
            current_dir.join("data/mdn_compat_data.json"),
            current_dir.join("data/mdn_css_properties.json"),
            current_dir.join("data/mdn_css_syntaxes.json"),
            current_dir.join("../../silex_html/src/tags"),
            current_dir.join("../../silex_css/src/codegen"),
            current_dir.join("../../silex_macros/src/css/tw/resolver/codegen"),
            current_dir.join("../../silex_macros/src/css"),
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
    println!("Macro Codegen dir: {}", macro_codegen_dir.display());

    // 2. FETCH MODE: Raw data downloader
    if should_fetch {
        println!("\n[FETCH MODE] Fetching raw data from MDN...");

        // Simple synchronous fetch utility
        let client = Client::builder().user_agent("silex-codegen").build()?;

        let fetch_and_save = |url: &str, path: &Path| -> Result<(), Box<dyn Error>> {
            println!("Downloading from {} ...", url);
            let response = client.get(url).send()?.error_for_status()?;
            let value: Value = from_reader(response)?;
            let file = File::create(path)?;
            let writer = BufWriter::new(file);
            to_writer_pretty(writer, &value)?;
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
    let compat_str = read_to_string(&mdn_compat_path)?;
    let props_str = read_to_string(&mdn_props_path)?;
    let syntaxes_str = read_to_string(&mdn_syntaxes_path)?;

    let config = parse_tags(&compat_str)?;
    let css_config = parse_css(&props_str, &syntaxes_str)?;

    println!("[CODEGEN MODE] Applying in-memory patches...");
    let mut gen_config = config.clone();
    apply_memory_only_patches(&mut gen_config);

    // 4. Generate and Write Rust Code
    if !out_dir.exists() {
        create_dir_all(&out_dir)?;
    }
    if !css_out_dir.exists() {
        create_dir_all(&css_out_dir)?;
    }

    // --- CSS Codegen ---
    let properties_code = generate_properties_macro(&css_config.properties);
    write(css_out_dir.join("properties.rs"), properties_code)?;
    println!("Generated properties.rs");

    let keywords_code = generate_keywords_code(&css_config.properties, &css_config.color_keywords);
    write(css_out_dir.join("keywords_gen.rs"), keywords_code)?;
    println!("Generated keywords_gen.rs");

    let property_names_code = generate_property_names_code(&css_config.properties);
    write(macro_css_dir.join("property_names.rs"), property_names_code)?;
    println!("Generated property_names.rs");

    // Generate HTML module
    let html_code = generate_module_content(&gen_config.html, false, &[]);
    write(out_dir.join("html.rs"), html_code)?;
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
    write(out_dir.join("svg.rs"), svg_code)?;
    println!("Generated svg.rs");

    // Generate Tailwind Classes & Macro Table
    let tw_data_dir = current_dir.join("crates/utils/silex_codegen/data/tailwind");
    if tw_data_dir.exists() {
        let classes_str = read_to_string(tw_data_dir.join("classes.json"))?;
        let dynamic_prefixes_str = read_to_string(tw_data_dir.join("dynamic_prefixes.json"))?;
        let prefix_metadata_str = read_to_string(tw_data_dir.join("prefix_metadata.json"))?;
        let test_cases_str = read_to_string(tw_data_dir.join("test_cases.json"))?;
        let palette_str = read_to_string(tw_data_dir.join("palette.json"))?;
        let modifiers_str = read_to_string(tw_data_dir.join("modifiers.json"))?;
        let keyframes_str = read_to_string(tw_data_dir.join("keyframes.json"))?;
        let extra_properties_str = read_to_string(tw_data_dir.join("extra_properties.json"))?;
        let property_aliases_str = read_to_string(tw_data_dir.join("property_aliases.json"))?;
        let reference_css_str = read_to_string(tw_data_dir.join("reference_css.json"))?;

        let classes: Vec<String> = from_str(&classes_str)?;
        let dynamic_prefixes: BTreeMap<String, Vec<String>> = from_str(&dynamic_prefixes_str)?;
        let prefix_metadata: BTreeMap<String, crate::tw::PrefixMetaJson> =
            from_str(&prefix_metadata_str)?;
        let test_cases: Vec<String> = from_str(&test_cases_str)?;
        let palette_data: BTreeMap<String, Vec<crate::tw::ColorShadeInfo>> =
            from_str(&palette_str)?;
        let modifiers_data: Vec<crate::tw::ModifierMetaJson> = from_str(&modifiers_str)?;
        let keyframes_data: Vec<crate::tw::KeyframeMetaJson> = from_str(&keyframes_str)?;
        let extra_properties: Vec<String> = from_str(&extra_properties_str)?;
        let property_aliases: BTreeMap<String, Vec<String>> = from_str(&property_aliases_str)?;
        let reference_css: ReferenceCssJson = from_str(&reference_css_str)?;

        // 生成前置校验闸门：任何非法元数据都必须让构建失败，而不是静默生成垃圾 CSS
        validate_prefix_metadata(&prefix_metadata)?;
        validate_resolver_properties(&classes, &test_cases, &palette_data, &props_str)?;
        validate_resolver_values(&classes, &test_cases, &palette_data)?;

        // 上游数据漂移闸门：大规模变更必须人工确认一次，不能静默生成
        let baseline_path = tw_data_dir.join("codegen_baseline.json");
        let baseline: CodegenBaseline = if baseline_path.exists() {
            from_str(&read_to_string(&baseline_path)?)?
        } else {
            CodegenBaseline::new()
        };
        let current = fingerprint_tw_datasets(
            &classes,
            &dynamic_prefixes,
            &prefix_metadata,
            &test_cases,
            &palette_data,
            &modifiers_data,
            &keyframes_data,
            &reference_css,
        );
        for notice in check_drift(&baseline, &current, accept_drift)? {
            println!("{notice}");
        }
        write(&baseline_path, serde_json::to_string_pretty(&current)?)?;

        let (table_code, table_unimplement_code) =
            generate_macro_tables(&classes, &dynamic_prefixes, &palette_data);
        let table_examples_code = generate_table_examples(&test_cases, &palette_data);
        let property_id_code = generate_property_id_code(
            &props_str,
            &classes,
            &test_cases,
            &palette_data,
            &extra_properties,
            &property_aliases,
            &prefix_metadata,
        );
        let prefix_metadata_code = generate_prefix_metadata_code(&prefix_metadata);
        let palette_code = generate_palette_code(&palette_data);
        let modifiers_code = generate_modifiers_code(&modifiers_data);
        let keyframes_code = generate_keyframes_code(&keyframes_data);
        let reference_css_code = generate_reference_css_code(&reference_css);

        if !macro_codegen_dir.exists() {
            create_dir_all(&macro_codegen_dir)?;
        }
        write(macro_codegen_dir.join("table.rs"), table_code)?;
        write(
            macro_codegen_dir.join("table_unimplement.rs"),
            table_unimplement_code,
        )?;
        write(
            macro_codegen_dir.join("table_examples.rs"),
            table_examples_code,
        )?;
        write(macro_codegen_dir.join("property_id.rs"), property_id_code)?;
        write(
            macro_codegen_dir.join("prefix_metadata.rs"),
            prefix_metadata_code,
        )?;
        write(macro_codegen_dir.join("palette.rs"), palette_code)?;
        write(macro_codegen_dir.join("modifiers.rs"), modifiers_code)?;
        write(macro_codegen_dir.join("keyframes.rs"), keyframes_code)?;
        write(
            macro_codegen_dir.join("reference_css.rs"),
            reference_css_code,
        )?;
        println!(
            "Generated table.rs, table_unimplement.rs, table_examples.rs, property_id.rs, prefix_metadata.rs, palette.rs, modifiers.rs, keyframes.rs and reference_css.rs for silex_macros in resolver/codegen"
        );
    }

    println!("\nSuccessfully completed!");
    Ok(())
}
