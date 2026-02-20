pub mod compiler;
pub mod style;
pub mod styled;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{LitStr, Result};

use compiler::CssCompiler;

pub fn css_impl(input: LitStr) -> Result<TokenStream> {
    let css_content = input.value();
    let compile_result = CssCompiler::compile(&css_content, input.span())?;

    let class_name = compile_result.class_name;
    let style_id = compile_result.style_id;
    let final_css = compile_result.final_css;
    let expressions = compile_result.expressions;
    let hash = compile_result.hash;

    // Generate Rust Code
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
