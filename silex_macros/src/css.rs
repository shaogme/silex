pub mod compiler;
pub mod style;
pub mod styled;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{LitStr, Result};

use compiler::CssCompiler;

pub fn css_impl(input: LitStr) -> Result<TokenStream> {
    let css_content = input.value();
    let ts = syn::parse_str::<TokenStream>(&css_content)?;
    let compile_result = CssCompiler::compile(ts, input.span())?;

    let class_name = compile_result.class_name;
    let style_id = compile_result.style_id;
    let final_css = compile_result.final_css;
    let expressions = compile_result.expressions;
    let dynamic_rules = compile_result.dynamic_rules;
    let hash = compile_result.hash;

    // Generate Rust Code
    if expressions.is_empty() && dynamic_rules.is_empty() {
        // Backward compatibility: return static class name string if no dynamics used
        Ok(quote! {
            {
                ::silex::css::inject_style(#style_id, #final_css);
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
                    (#var_name, ::silex::css::make_dynamic_val(#expr))
                }
            })
            .collect();

        let rule_decls: Vec<TokenStream> = dynamic_rules
            .iter()
            .map(|rule| {
                let template = &rule.template;
                let exprs: Vec<TokenStream> = rule
                    .expressions
                    .iter()
                    .map(|expr| {
                        quote! { ::silex::css::make_dynamic_val(#expr) }
                    })
                    .collect();

                quote! {
                    (#template, ::std::vec![ #(#exprs),* ])
                }
            })
            .collect();

        Ok(quote! {
            {
                ::silex::css::inject_style(#style_id, #final_css);
                ::silex::css::DynamicCss {
                    class_name: #class_name,
                    vars: ::std::vec![
                        #(#var_decls),*
                    ],
                    rules: ::std::vec![
                        #(#rule_decls),*
                    ]
                }
            }
        })
    }
}
