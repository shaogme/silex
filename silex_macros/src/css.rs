pub mod ast;
pub mod compiler;
pub mod error;
pub mod style;
pub mod styled;
pub mod theme;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{LitStr, Result};

use compiler::CssCompiler;

macro_rules! define_properties {
    ($($css_name:literal => $rust_type:ident),* $(,)?) => {
        fn lookup_builtin_prop(prop: &str) -> Option<&'static str> {
            match prop {
                $($css_name => Some(stringify!($rust_type)),)*
                _ => None,
            }
        }
    };
}

// 核心属性映射表
define_properties! {
    "any" => Any,
    "width" => Width,
    "height" => Height,
    "color" => Color,
    "background-color" => BackgroundColor,
    "margin" => Margin,
    "padding" => Padding,
    "display" => Display,
    "position" => Position,
    "z-index" => ZIndex,
    "opacity" => Opacity,
    "flex" => Flex,
    "grid" => Grid,
}

pub(crate) fn get_prop_type(prop: &str, span: Span) -> Result<TokenStream> {
    // 1. 优先查表
    if let Some(type_name) = lookup_builtin_prop(prop) {
        let ident = syn::Ident::new(type_name, Span::call_site());
        return Ok(quote_spanned! { span => ::silex::css::types::props::#ident });
    }

    // 2. 自动化转换：kebab-case -> PascalCase (作为回退)
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

    let ident = syn::Ident::new(&pascal, Span::call_site());
    Ok(quote_spanned! { span => ::silex::css::types::props::#ident })
}

pub fn css_impl(input: LitStr) -> Result<TokenStream> {
    let css_content = input.value();
    let ts = syn::parse_str::<TokenStream>(&css_content)?;
    let compile_result = CssCompiler::compile(ts, input.span(), None)?;

    let class_name = compile_result.class_name;
    let style_id = compile_result.style_id;
    let static_css = compile_result.static_css;
    let component_css = compile_result.component_css;
    let expressions = compile_result.expressions;
    let dynamic_rules = compile_result.dynamic_rules;
    let theme_refs = compile_result.theme_refs;

    let theme_assertions: Vec<TokenStream> = theme_refs
        .iter()
        .map(|(prop, key)| -> Result<TokenStream> {
            let prop_type = get_prop_type(prop, input.span())?;
            let key_path: Vec<TokenStream> = key
                .split('.')
                .map(|s| {
                    let id = quote::format_ident!("{}", s);
                    quote! { #id }
                })
                .collect();

            Ok(quote! {
                const _: () = {
                    fn assert_valid<V: ::silex::css::types::ValidFor<#prop_type>>(_: &V) {}
                    #[allow(non_upper_case_globals, unused_variables)]
                    let _ = |t: &Theme| {
                        assert_valid(&t #(.#key_path)*);
                    };
                };
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let static_id = compile_result.static_id;
    let inits = quote! {
        #(#theme_assertions)*
        if !#static_css.is_empty() {
            ::silex::css::inject_style(#static_id, #static_css);
        }
        if !#component_css.is_empty() {
            ::silex::css::inject_style(#style_id, #component_css);
        }
    };

    // Generate Rust Code
    if expressions.is_empty() && dynamic_rules.is_empty() {
        Ok(quote! {
            {
                #inits
                #class_name
            }
        })
    } else {
        // Generate DynamicCss struct
        let mut var_decls = Vec::new();
        for (i, (prop, expr)) in expressions.iter().enumerate() {
            let var_name = format!("--{}-{}", class_name, i);
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
                #inits
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
