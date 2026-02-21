pub mod ast;
pub mod compiler;
pub mod style;
pub mod styled;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{LitStr, Result};

use compiler::CssCompiler;

pub(crate) fn get_prop_type(
    prop: &str,
    span: proc_macro2::Span,
) -> Result<proc_macro2::TokenStream> {
    if prop == "any" {
        return Ok(quote::quote_spanned! { span => ::silex::css::types::props::Any });
    }
    let supported = [
        "width",
        "height",
        "margin",
        "padding",
        "color",
        "background",
        "background-color",
        "z-index",
        "display",
        "position",
        "flex-direction",
        "flex-wrap",
        "flex-grow",
        "flex-shrink",
        "flex-basis",
        "align-items",
        "justify-content",
        "background-image",
        "border",
        "border-width",
        "border-style",
        "border-color",
        "border-radius",
        "font-size",
        "font-weight",
        "letter-spacing",
        "line-height",
        "text-align",
        "text-decoration",
        "cursor",
        "gap",
        "top",
        "left",
        "right",
        "bottom",
        "opacity",
        "visibility",
        "pointer-events",
        "overflow",
        "overflow-x",
        "overflow-y",
        "transition",
        "transform",
        "box-shadow",
        "backdrop-filter",
        "filter",
        "outline",
    ];
    if supported.contains(&prop) {
        let pascal: String = prop
            .split('-')
            .map(|part| {
                let mut c = part.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect();
        let ident = syn::Ident::new(&pascal, proc_macro2::Span::call_site());
        Ok(quote::quote_spanned! { span => ::silex::css::types::props::#ident })
    } else {
        Err(syn::Error::new(
            span,
            format!(
                "Unsupported dynamic CSS property: `{}`. If you need to use this property with interpolation, you may need to use UnsafeCss or update the framework.",
                prop
            ),
        ))
    }
}

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
        let mut var_decls = Vec::new();
        for (i, (prop, expr)) in expressions.iter().enumerate() {
            let var_name = format!("--slx-{:x}-{}", hash, i);
            let prop_type = get_prop_type(prop, input.span())?;
            var_decls.push(quote! {
                (#var_name, ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr))
            });
        }

        let mut rule_decls = Vec::new();
        for rule in &dynamic_rules {
            let template = &rule.template;
            let mut exprs = Vec::new();
            for (prop, expr) in &rule.expressions {
                let prop_type = get_prop_type(prop, input.span())?;
                exprs.push(quote! { ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr) });
            }
            rule_decls.push(quote! {
                (#template, ::std::vec![ #(#exprs),* ])
            });
        }

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
