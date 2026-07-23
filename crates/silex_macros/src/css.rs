pub mod ast;
pub mod classes;
pub mod compiler;
pub mod config;
pub mod error;
pub mod styled;
pub mod table;
pub mod theme;
#[cfg(feature = "tw")]
pub mod tw;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::Result;

use compiler::CssCompiler;
use table::PropertyResolveResult;

pub(crate) fn get_prop_type(prop: &str, span: Span) -> Result<TokenStream> {
    match table::resolve_property_type(prop, span)? {
        PropertyResolveResult::Builtin(type_name) => {
            let ident = syn::Ident::new(type_name, Span::call_site());
            Ok(quote_spanned! { span => ::silex::css::types::props::#ident })
        }
        PropertyResolveResult::CustomVar => {
            Ok(quote_spanned! { span => ::silex::css::types::props::Any })
        }
    }
}

pub fn inject_css_impl(ts: TokenStream) -> Result<TokenStream> {
    let span = Span::call_site();
    let compile_result = CssCompiler::compile_global(ts, span, false)?;
    let static_id = &compile_result.static_id;
    let static_css = &compile_result.static_css;
    let style_id = &compile_result.style_id;
    let component_css = &compile_result.component_css;

    Ok(quote! {
        {
            const __STATIC_CSS: &str = #static_css;
            const __COMPONENT_CSS: &str = #component_css;

            if !__STATIC_CSS.is_empty() {
                ::silex::css::inject_style(#static_id, __STATIC_CSS);
            }
            if !__COMPONENT_CSS.is_empty() {
                ::silex::css::inject_style(#style_id, __COMPONENT_CSS);
            }
        }
    })
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
    let style_inits = compile_result.generate_inits();
    let class_name = compile_result.class_name;
    let expressions = compile_result.expressions;
    let dynamic_rules = compile_result.dynamic_rules;
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
        #style_inits
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_prop_type_custom_variable_and_builtin() {
        let span = Span::call_site();
        let builtin = get_prop_type("p", span).unwrap().to_string();
        assert!(builtin.contains("Padding"));

        let standard = get_prop_type("grid-template-columns", span).unwrap().to_string();
        assert!(standard.contains("GridTemplateColumns"));

        let custom_var = get_prop_type("--my-custom-color", span).unwrap().to_string();
        assert!(custom_var.contains("Any"));

        let unknown_err = get_prop_type("invalid-unknown-prop", span);
        assert!(unknown_err.is_err());
        assert!(unknown_err.unwrap_err().to_string().contains("未知的 CSS 属性"));
    }
}
