use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use syn::{LitStr, Result};

pub fn css_impl(input: LitStr) -> Result<TokenStream> {
    let css_content = input.value();

    // 1. Calculate Hash
    let mut hasher = DefaultHasher::new();
    css_content.hash(&mut hasher);
    let hash = hasher.finish();
    let class_name = format!("slx-{:x}", hash);
    let style_id = format!("style-{}", class_name);

    // 2. Wrap Content
    // We wrap the content in the class selector to form a valid stylesheet rule.
    // This allows users to write declarations directly or use nesting.
    let wrapped_css = format!(".{} {{ {} }}", class_name, css_content);

    // 3. Parse & Validate
    // Use lightningcss to parse the stylesheet. This validates the syntax.
    // We use a dummy filename "silex_generated.css".
    let mut stylesheet = StyleSheet::parse(&wrapped_css, ParserOptions::default())
        .map_err(|e| syn::Error::new(input.span(), format!("Invalid CSS: {}", e)))?;

    // 4. Minify
    // Optimize the CSS for size.
    stylesheet
        .minify(MinifyOptions::default())
        .map_err(|e| syn::Error::new(input.span(), format!("CSS Minification failed: {}", e)))?;

    // 5. Generate Output CSS
    let res = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            targets: Targets::default(), // Default targets (modern browsers)
            ..PrinterOptions::default()
        })
        .map_err(|e| syn::Error::new(input.span(), format!("CSS Printing failed: {}", e)))?;

    let final_css = res.code;

    // 6. Generate Rust Code
    let output = quote! {
        {
            silex::css::inject_style(#style_id, #final_css);
            #class_name
        }
    };

    Ok(output)
}
