pub mod ast;
pub mod codegen;
pub mod parser;
pub mod resolver;

use ast::TwInput;
use codegen::build_css_block_from_tw;
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
    let input_str = if verbose {
        ts.to_string()
    } else {
        String::new()
    };
    let input: TwInput = syn::parse2(ts)?;
    let extra_classes = input.extra_classes.clone();
    let css_block = build_css_block_from_tw(input)?;

    // 将组装好的 Silex CssBlock 转化为 TokenStream
    let block_ts = quote! { #css_block };
    let span = proc_macro2::Span::call_site();

    // 接入 silex_macros::css 编译管线，自动享受 LightningCSS 压缩与哈希 Class 注入
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

    crate::css::generate_css_output(compile_result, span)
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
}
