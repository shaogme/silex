pub mod ast;
pub mod classes;
pub mod compiler;
pub mod config;
pub mod error;
pub mod styled;
pub mod theme;
#[cfg(feature = "tw")]
pub mod tw;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::Result;

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
    "p" => Padding,
    "px" => PaddingInline,
    "py" => PaddingBlock,
    "pt" => PaddingTop,
    "pr" => PaddingRight,
    "pb" => PaddingBottom,
    "pl" => PaddingLeft,
    "m" => Margin,
    "mx" => MarginInline,
    "my" => MarginBlock,
    "mt" => MarginTop,
    "mr" => MarginRight,
    "mb" => MarginBottom,
    "ml" => MarginLeft,
    "w" => Width,
    "h" => Height,
    "bg" => BackgroundColor,
    "text" => Color,
    "border" => BorderColor,
    "rounded" => BorderRadius,
    "width" => Width,
    "height" => Height,
    "color" => Color,
    "background-color" => BackgroundColor,
    "margin" => Margin,
    "padding" => Padding,
    "padding-inline" => PaddingInline,
    "padding-block" => PaddingBlock,
    "padding-top" => PaddingTop,
    "padding-right" => PaddingRight,
    "padding-bottom" => PaddingBottom,
    "padding-left" => PaddingLeft,
    "margin-inline" => MarginInline,
    "margin-block" => MarginBlock,
    "margin-top" => MarginTop,
    "margin-right" => MarginRight,
    "margin-bottom" => MarginBottom,
    "margin-left" => MarginLeft,
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

    // 2. 严谨按 PascalCase 规则映射到 silex_css::types::props 中对应的强类型 Struct (禁止回退到 Any)
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

pub fn css_impl(ts: TokenStream) -> Result<TokenStream> {
    let span = Span::call_site(); // Use call site for better error reporting in blocks
    let compile_result = CssCompiler::compile(ts, span, false)?;
    generate_css_output(compile_result, span)
}

pub(crate) fn generate_css_output(
    compile_result: compiler::CssCompileResult,
    span: Span,
) -> Result<TokenStream> {
    let class_name = compile_result.class_name;
    let style_id = compile_result.style_id;
    let static_css = compile_result.static_css;
    let component_css = compile_result.component_css;
    let expressions = compile_result.expressions;
    let dynamic_rules = compile_result.dynamic_rules;

    let static_id = compile_result.static_id;
    let warnings = compile_result.warnings;
    let warning_tokens = warnings.iter().map(|w| {
        let msg = &w.message;
        let warning_span = w.span;
        quote_spanned! { warning_span =>
            #[allow(non_upper_case_globals, dead_code)]
            #[deprecated(note = #msg)]
            const _SILEX_CSS_WARNING: () = ();
            let _ = _SILEX_CSS_WARNING;
        }
    });

    let inits = quote! {
        #(#warning_tokens)*
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
            let prop_type = get_prop_type(prop, span)?;
            var_decls.push(quote! {
                (#var_name, ::silex::css::make_dynamic_val_for::<#prop_type, _>(#expr))
            });
        }

        let mut rule_decls = Vec::new();
        for rule in &dynamic_rules {
            let template = &rule.template;
            let mut exprs = Vec::new();
            for (prop, expr) in &rule.expressions {
                let prop_type = get_prop_type(prop, span)?;
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
