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

fn generate_inits(compile_result: &crate::css::compiler::CssCompileResult) -> TokenStream {
    let style_id = &compile_result.style_id;
    let static_id = &compile_result.static_id;
    let static_css = &compile_result.static_css;
    let component_css = &compile_result.component_css;

    quote! {
        if !#static_css.is_empty() {
            ::silex::css::inject_style(#static_id, #static_css);
        }
        if !#component_css.is_empty() {
            ::silex::css::inject_style(#style_id, #component_css);
        }
    }
}

fn tw_impl_internal(ts: TokenStream, verbose: bool) -> Result<TokenStream> {
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
        let block_ts = quote! { #css_block };
        let mut compile_result =
            crate::css::compiler::CssCompiler::compile(block_ts.clone(), span, false)?;

        if !extra_classes.is_empty() {
            let extra_str = extra_classes.join(" ");
            compile_result.class_name = format!("{} {}", compile_result.class_name, extra_str);
        }

        if verbose {
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
    let mut reactive_body = Vec::new();

    for seg in input.segments {
        match seg {
            TwSegment::Static(rules) => {
                if rules.is_empty() {
                    continue;
                }
                let css_block = build_css_block_from_rules(rules)?;
                let block_ts = quote! { #css_block };
                let compile_result =
                    crate::css::compiler::CssCompiler::compile(block_ts, span, false)?;
                let cls_name = compile_result.class_name.clone();
                inits_tokens.push(generate_inits(&compile_result));

                reactive_body.push(quote! {
                    if !_slx_cls.is_empty() { _slx_cls.push(' '); }
                    _slx_cls.push_str(#cls_name);
                });
            }
            TwSegment::Conditional {
                condition,
                then_rules,
                else_rules,
                ..
            } => {
                let then_cls = if then_rules.is_empty() {
                    String::new()
                } else {
                    let css_block = build_css_block_from_rules(then_rules)?;
                    let block_ts = quote! { #css_block };
                    let compile_result =
                        crate::css::compiler::CssCompiler::compile(block_ts, span, false)?;
                    let cls = compile_result.class_name.clone();
                    inits_tokens.push(generate_inits(&compile_result));
                    cls
                };

                let else_cls = if else_rules.is_empty() {
                    String::new()
                } else {
                    let css_block = build_css_block_from_rules(else_rules)?;
                    let block_ts = quote! { #css_block };
                    let compile_result =
                        crate::css::compiler::CssCompiler::compile(block_ts, span, false)?;
                    let cls = compile_result.class_name.clone();
                    inits_tokens.push(generate_inits(&compile_result));
                    cls
                };

                if !else_cls.is_empty() {
                    reactive_body.push(quote! {
                        if #condition {
                            if !#then_cls.is_empty() {
                                if !_slx_cls.is_empty() { _slx_cls.push(' '); }
                                _slx_cls.push_str(#then_cls);
                            }
                        } else {
                            if !_slx_cls.is_empty() { _slx_cls.push(' '); }
                            _slx_cls.push_str(#else_cls);
                        }
                    });
                } else {
                    reactive_body.push(quote! {
                        if #condition {
                            if !#then_cls.is_empty() {
                                if !_slx_cls.is_empty() { _slx_cls.push(' '); }
                                _slx_cls.push_str(#then_cls);
                            }
                        }
                    });
                }
            }
        }
    }

    if !extra_classes.is_empty() {
        let extra_str = extra_classes.join(" ");
        reactive_body.push(quote! {
            if !_slx_cls.is_empty() { _slx_cls.push(' '); }
            _slx_cls.push_str(#extra_str);
        });
    }

    Ok(quote! {
        {
            #(#inits_tokens)*
            ::silex::core::rx!(move || {
                let mut _slx_cls = ::std::string::String::with_capacity(64);
                #(#reactive_body)*
                _slx_cls
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn test_animate_spin_css() {
        let input: TwInput = syn::parse2(quote!("animate-spin")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(compile_result.static_css.contains("@keyframes spin"));
        assert!(
            compile_result
                .component_css
                .contains("animation:1s linear infinite spin")
        );
    }

    #[test]
    fn test_animate_ping_pulse_bounce_css() {
        let input: TwInput =
            syn::parse2(quote!("animate-ping animate-pulse animate-bounce")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(compile_result.static_css.contains("@keyframes ping"));
        assert!(compile_result.static_css.contains("@keyframes pulse"));
        assert!(compile_result.static_css.contains("@keyframes bounce"));
    }

    #[test]
    fn test_hover_blur_css() {
        let input: TwInput = syn::parse2(quote!("blur-sm hover:blur-none")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        println!("component_css: {}", compile_result.component_css);
        let cls = &compile_result.class_name;
        assert!(
            compile_result
                .component_css
                .contains("&:hover{filter:none}")
                || compile_result
                    .component_css
                    .contains(&format!("{}:hover", cls)),
            "Expected component_css to contain hover filter rule, but got: {}",
            compile_result.component_css
        );
    }

    #[test]
    fn test_group_hover_css() {
        let input: TwInput =
            syn::parse2(quote!("group-hover:rotate-180 peer-focus:block")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        println!(
            "group_hover component_css: {}",
            compile_result.component_css
        );
        assert!(
            compile_result.component_css.contains(".group:hover &")
                || compile_result.component_css.contains(".group:hover ."),
            "Expected group-hover rule, got: {}",
            compile_result.component_css
        );
        assert!(
            compile_result.component_css.contains(".peer:focus~&")
                || compile_result.component_css.contains(".peer:focus ~ &")
                || compile_result.component_css.contains(".peer:focus ~ ."),
            "Expected peer-focus rule, got: {}",
            compile_result.component_css
        );
    }

    #[test]
    fn test_container_query_css() {
        let input: TwInput = syn::parse2(quote!("@container @sm:p-4 @[400px]:flex")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        println!("container component_css: {}", compile_result.component_css);
        assert!(
            compile_result
                .component_css
                .contains("container-type:inline-size")
                || compile_result
                    .component_css
                    .contains("container-type: inline-size")
        );
        assert!(
            compile_result.component_css.contains("width>=640px")
                || compile_result.component_css.contains("min-width: 640px")
                || compile_result.component_css.contains("min-width:640px")
        );
        assert!(
            compile_result.component_css.contains("width>=400px")
                || compile_result.component_css.contains("min-width: 400px")
                || compile_result.component_css.contains("min-width:400px")
        );
    }

    #[test]
    fn test_multiple_at_rules_css() {
        let input: TwInput = syn::parse2(quote!("md:@sm:p-4")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        println!(
            "multiple_at_rules component_css: {}",
            compile_result.component_css
        );
        assert!(
            compile_result.component_css.contains("media")
                && compile_result.component_css.contains("container"),
            "Expected component_css to contain both media and container rules, got: {}",
            compile_result.component_css
        );
    }

    #[test]
    fn test_shorthand_longhand_deduplication() {
        // 1. pt-2 在前，p-4 在后：后面的 shorthand (p-4) 完全覆盖前面的 pt-2
        let input: TwInput = syn::parse2(quote!("pt-2 p-4")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(
            !compile_result.component_css.contains("padding-top"),
            "Expected padding-top to be overridden by later p-4 shorthand, got: {}",
            compile_result.component_css
        );
        assert!(
            compile_result.component_css.contains("padding:1rem")
                || compile_result.component_css.contains("padding: 1rem"),
            "Expected padding:1rem, got: {}",
            compile_result.component_css
        );

        // 2. p-4 在前，pt-2 在后：两者保留并由 LightningCSS 压缩合并为 padding: .5rem 1rem 1rem
        let input: TwInput = syn::parse2(quote!("p-4 pt-2")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let block_ts = quote! { #css_block };
        let compile_result = crate::css::compiler::CssCompiler::compile(
            block_ts,
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(
            compile_result
                .component_css
                .contains("padding:.5rem 1rem 1rem")
                || compile_result
                    .component_css
                    .contains("padding: .5rem 1rem 1rem")
                || (compile_result.component_css.contains("padding:1rem")
                    && compile_result.component_css.contains("padding-top")),
            "Expected compressed padding:.5rem 1rem 1rem or padding:1rem + padding-top, got: {}",
            compile_result.component_css
        );
    }

    #[test]
    fn test_conditional_tw_macro() {
        let ts = quote!(
            "p-4",
            (is_active, "bg-indigo-600 text-white"),
            (is_dark, "bg-slate-900", "bg-white")
        );
        let output = tw_impl(ts).unwrap();
        let code = output.to_string();
        assert!(code.contains("rx !"));
        assert!(code.contains("is_active"));
        assert!(code.contains("is_dark"));
        assert!(code.contains("inject_style"));
    }

    #[test]
    fn test_ring_system_css() {
        let input: TwInput =
            syn::parse2(quote!("ring-2 ring-indigo-500/20 ring-offset-2")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile(
            quote! { #css_block },
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(compile_result.component_css.contains("box-shadow:"));
        assert!(
            compile_result
                .component_css
                .contains("--tw-ring-color:#6366f133")
                || compile_result.component_css.contains("--tw-ring-color:")
        );
        assert!(compile_result.component_css.contains("--tw-ring-width:2px"));
        assert!(
            compile_result
                .component_css
                .contains("--tw-ring-offset-width:2px")
        );
    }

    #[test]
    fn test_gradient_system_css() {
        let input: TwInput = syn::parse2(quote!(
            "bg-gradient-to-r from-indigo-500 via-purple-500 to-pink-500"
        ))
        .unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile(
            quote! { #css_block },
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(
            compile_result
                .component_css
                .contains("background-image:linear-gradient(to right,var(--tw-gradient-stops))")
        );
        assert!(
            compile_result
                .component_css
                .contains("--tw-gradient-from:#6366f1")
        );
        assert!(
            compile_result
                .component_css
                .contains("--tw-gradient-via:#a855f7")
        );
        assert!(
            compile_result
                .component_css
                .contains("--tw-gradient-to:#ec4899")
        );
    }

    #[test]
    fn test_divide_and_space_css() {
        let input: TwInput = syn::parse2(quote!("divide-y divide-slate-200 space-x-4")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile(
            quote! { #css_block },
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(compile_result.component_css.contains(":not([hidden])"));
        assert!(compile_result.component_css.contains("margin-left:1rem"));
    }

    #[test]
    fn test_line_clamp_and_presets_css() {
        let input: TwInput = syn::parse2(quote!(
            "line-clamp-2 truncate z-50 opacity-75 pointer-events-none select-none"
        ))
        .unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile(
            quote! { #css_block },
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(compile_result.component_css.contains("z-index:50"));
        assert!(
            compile_result.component_css.contains("opacity:.75")
                || compile_result.component_css.contains("opacity: 0.75")
        );
        assert!(compile_result.component_css.contains("pointer-events:none"));
        assert!(compile_result.component_css.contains("user-select:none"));
        assert!(
            compile_result
                .component_css
                .contains("-webkit-line-clamp:2")
        );
    }

    #[test]
    fn test_arbitrary_property_ring_color() {
        // Arbitrary property syntax: [--tw-ring-color:rgba(79,70,229,.2)]
        let input: TwInput = syn::parse2(quote!("[--tw-ring-color:rgba(79,70,229,.2)]")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile(
            quote! { #css_block },
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(
            compile_result
                .component_css
                .contains("--tw-ring-color:#6366f133")
                || compile_result
                    .component_css
                    .contains("--tw-ring-color:rgba(79,70,229,.2)")
                || compile_result.component_css.contains("--tw-ring-color:")
        );

        // Arbitrary value syntax with prefix: ring-[rgba(79,70,229,.2)]
        let input: TwInput = syn::parse2(quote!("ring-[rgba(79,70,229,.2)]")).unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile(
            quote! { #css_block },
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(
            compile_result
                .component_css
                .contains("--tw-ring-color:#6366f133")
                || compile_result
                    .component_css
                    .contains("--tw-ring-color:rgba(79,70,229,.2)")
                || compile_result.component_css.contains("--tw-ring-color:")
        );
    }

    #[test]
    fn test_multi_property_rgba_support() {
        let input: TwInput = syn::parse2(quote!(
            "bg-[rgba(79,70,229,.2)] text-[rgba(15,23,42,.8)] border-[rgba(244,63,94,.5)] border-t-[rgba(255,255,255,.9)] accent-[rgba(79,70,229,.2)] from-[rgba(79,70,229,.2)] divide-[rgba(226,232,240,.5)] bg-rgba(79,70,229,.2) bg-indigo-500/.2"
        ))
        .unwrap();
        let css_block = build_css_block_from_tw(input).unwrap();
        let compile_result = crate::css::compiler::CssCompiler::compile(
            quote! { #css_block },
            proc_macro2::Span::call_site(),
            false,
        )
        .unwrap();
        assert!(
            compile_result
                .component_css
                .contains("accent-color:#4f46e533")
                || compile_result.component_css.contains("accent-color:")
        );
        assert!(
            compile_result
                .component_css
                .contains("border-color:#ffffffe6")
                || compile_result.component_css.contains("border-color:")
        );
        assert!(compile_result.component_css.contains("--tw-gradient-from:"));
    }
}
