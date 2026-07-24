use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use quote::quote;

#[test]
fn test_animate_spin_css() {
    let input: TwInput = syn::parse2(quote!("animate-spin")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let block_ts = quote! { #css_block };
    let compile_result =
        crate::css::compiler::CssCompiler::compile(block_ts, proc_macro2::Span::call_site(), false)
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
    let input: TwInput = syn::parse2(quote!("animate-ping animate-pulse animate-bounce")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let block_ts = quote! { #css_block };
    let compile_result =
        crate::css::compiler::CssCompiler::compile(block_ts, proc_macro2::Span::call_site(), false)
            .unwrap();
    assert!(compile_result.static_css.contains("@keyframes ping"));
    assert!(compile_result.static_css.contains("@keyframes pulse"));
    assert!(compile_result.static_css.contains("@keyframes bounce"));
    assert!(compile_result.static_css.contains("cubic-bezier(.8,0,1,1)"));
    assert!(!compile_result.static_css.contains("cubic - bezier"));
}
