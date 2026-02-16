use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::Targets;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use syn::{LitStr, Result};

pub fn css_impl(input: LitStr) -> Result<TokenStream> {
    let css_content = input.value();

    // 1. Parse Interpolations
    let mut pre_css = String::new();
    let mut expressions = Vec::new();

    let mut chars = css_content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            if let Some(&'(') = chars.peek() {
                chars.next(); // consume '('

                let mut expr_str = String::new();
                let mut depth = 1;
                while let Some(inner_c) = chars.next() {
                    if inner_c == '(' {
                        depth += 1;
                    } else if inner_c == ')' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    expr_str.push(inner_c);
                }

                if depth != 0 {
                    return Err(syn::Error::new(
                        input.span(),
                        "Unbalanced parentheses in interpolation",
                    ));
                }

                let index = expressions.len();
                // Validate expression syntax early
                let expr: TokenStream = syn::parse_str(&expr_str)?;
                expressions.push(expr);

                // Use a temporary variable name that is valid in CSS
                pre_css.push_str(&format!("var(--slx-tmp-{})", index));
            } else {
                pre_css.push('$');
            }
        } else {
            pre_css.push(c);
        }
    }

    // 2. Calculate Hash (includes placeholders, so structure is hashed)
    let mut hasher = DefaultHasher::new();
    pre_css.hash(&mut hasher);
    let hash = hasher.finish();
    let class_name = format!("slx-{:x}", hash);
    let style_id = format!("style-{}", class_name);

    // 3. Replace Placeholders with Final Variable Names
    // The final variable name will include the hash to be unique to this scope
    // --slx-HASH-INDEX
    let mut final_source_css = pre_css.clone();
    for i in 0..expressions.len() {
        let placeholder = format!("--slx-tmp-{}", i);
        let real_var = format!("--slx-{:x}-{}", hash, i);
        final_source_css = final_source_css.replace(&placeholder, &real_var);
    }

    // 4. Wrap Content
    // We wrap the content in the class selector to form a valid stylesheet rule.
    // This allows users to write declarations directly or use nesting.
    let wrapped_css = format!(".{} {{ {} }}", class_name, final_source_css);

    // 5. Parse & Validate
    // Use lightningcss to parse the stylesheet. This validates the syntax.
    // We use a dummy filename "silex_generated.css".
    let mut stylesheet = StyleSheet::parse(&wrapped_css, ParserOptions::default())
        .map_err(|e| syn::Error::new(input.span(), format!("Invalid CSS: {}", e)))?;

    // 6. Minify
    // Optimize the CSS for size.
    stylesheet
        .minify(MinifyOptions::default())
        .map_err(|e| syn::Error::new(input.span(), format!("CSS Minification failed: {}", e)))?;

    // 7. Generate Output CSS
    let res = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            targets: Targets::default(), // Default targets (modern browsers)
            ..PrinterOptions::default()
        })
        .map_err(|e| syn::Error::new(input.span(), format!("CSS Printing failed: {}", e)))?;

    let final_css = res.code;

    // 8. Generate Rust Code
    if expressions.is_empty() {
        // Backward compatibility: return static class name string if no dynamics used
        Ok(quote! {
            {
                silex::css::inject_style(#style_id, #final_css);
                #class_name
            }
        })
    } else {
        // Generate DynamicCss struct
        let var_decls: Vec<TokenStream> = expressions
            .iter()
            .enumerate()
            .map(|(i, expr)| {
                let var_name = format!("--slx-{:x}-{}", hash, i);
                // Use the helper function to avoid type inference issues
                quote! {
                    (#var_name, silex::css::make_dynamic_val(#expr))
                }
            })
            .collect();

        Ok(quote! {
            {
                silex::css::inject_style(#style_id, #final_css);
                silex::css::DynamicCss {
                    class_name: #class_name,
                    vars: vec![
                        #(#var_decls),*
                    ]
                }
            }
        })
    }
}
