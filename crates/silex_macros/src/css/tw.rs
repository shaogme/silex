pub mod ast;
pub mod codegen;
pub mod parser;
pub mod resolver;
pub mod variants;

pub use variants::tw_variants_impl;

use ast::{TwInput, TwSegment};
use codegen::{build_css_block_from_rules, build_css_block_from_tw};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Result;

/// `tw!` 过程宏核心实现
pub fn tw_impl(ts: TokenStream) -> Result<TokenStream> {
    tw_impl_internal(ts, false)
}

/// `tw_verbose!` 过程宏核心实现 (带编译期 CSS 诊断打印)
pub fn tw_verbose_impl(ts: TokenStream) -> Result<TokenStream> {
    tw_impl_internal(ts, true)
}

fn tw_impl_internal(ts: TokenStream, verbose: bool) -> Result<TokenStream> {
    let __silex = crate::crate_path::silex();
    let input_str = if verbose {
        ts.to_string()
    } else {
        String::new()
    };
    let input: TwInput = syn::parse2(ts)?;
    let extra_classes = input.extra_classes.clone();
    let span = proc_macro2::Span::call_site();

    let has_conditionals = input
        .segments
        .iter()
        .any(|s| matches!(s, TwSegment::Conditional { .. }));

    if !has_conditionals {
        let css_block = build_css_block_from_tw(input)?;
        let mut compile_result =
            crate::css::compiler::CssCompiler::compile_block(&css_block, span, false)?;

        if !extra_classes.is_empty() {
            let extra_str = extra_classes.join(" ");
            compile_result.class_name = format!("{} {}", compile_result.class_name, extra_str);
        }

        if verbose {
            let block_ts = quote! { #css_block };
            eprintln!("========== [Silex tw_verbose! Compile-Time Diagnostics] ==========");
            eprintln!("Macro Input: {}", input_str);
            eprintln!("Generated CssBlock AST:\n  {}", block_ts);
            eprintln!("Compiled Class Name: {}", compile_result.class_name);
            eprintln!("Static CSS:\n  {}", compile_result.static_css);
            eprintln!("Component CSS:\n  {}", compile_result.component_css);
            eprintln!("====================================================================");
        }

        return crate::css::generate_css_output(compile_result, span);
    }

    // 处理包含条件分支句段的情形
    let mut inits_tokens = Vec::new();
    let mut cx_items = Vec::new();
    let mut compiled_cache = ::std::collections::HashMap::<u128, String>::new();

    let mut compile_rules_cached = |rules: Vec<ast::UtilityRule>| -> Result<String> {
        if rules.is_empty() {
            return Ok(String::new());
        }
        use silex_hash::css::CssHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher1 = CssHasher::with_seed(0x9e3779b97f4a7c15);
        let mut hasher2 = CssHasher::with_seed(0xbf58476d1ce4e5b9);
        rules.hash(&mut hasher1);
        rules.hash(&mut hasher2);
        let key = ((hasher1.finish() as u128) << 64) | (hasher2.finish() as u128);

        if let Some(cls) = compiled_cache.get(&key) {
            return Ok(cls.clone());
        }
        let css_block = build_css_block_from_rules(rules)?;
        let compile_result =
            crate::css::compiler::CssCompiler::compile_block(&css_block, span, false)?;
        let cls_name = compile_result.class_name.clone();
        inits_tokens.push(compile_result.generate_inits());
        compiled_cache.insert(key, cls_name.clone());
        Ok(cls_name)
    };

    for seg in input.segments {
        match seg {
            TwSegment::Static(rules) => {
                let cls_name = compile_rules_cached(rules)?;
                if !cls_name.is_empty() {
                    cx_items.push(quote! { #cls_name });
                }
            }
            TwSegment::Conditional {
                condition,
                then_rules,
                else_rules,
                ..
            } => {
                let then_cls = compile_rules_cached(then_rules)?;
                let else_cls = compile_rules_cached(else_rules)?;

                if !else_cls.is_empty() {
                    cx_items.push(quote! { (#condition, #then_cls, #else_cls) });
                } else {
                    cx_items.push(quote! { (#condition, #then_cls) });
                }
            }
        }
    }

    if !extra_classes.is_empty() {
        let extra_str = extra_classes.join(" ");
        cx_items.push(quote! { #extra_str });
    }

    Ok(quote! {
        {
            #(#inits_tokens)*
            #__silex::core::rx!(move || {
                #__silex::css::cx!(
                    #(#cx_items),*
                )
            })
        }
    })
}

#[cfg(test)]
mod tests;
