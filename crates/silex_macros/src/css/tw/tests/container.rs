use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use quote::quote;

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
fn test_named_container_query_css() {
    let input: TwInput =
        syn::parse2(quote!("@container/card-header @card-header/sm:p-4")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let block_ts = quote! { #css_block };
    let compile_result = crate::css::compiler::CssCompiler::compile(
        block_ts,
        proc_macro2::Span::call_site(),
        false,
    )
    .unwrap();
    println!(
        "named container component_css: {}",
        compile_result.component_css
    );
    assert!(
        compile_result
            .component_css
            .contains("container:card-header/inline-size")
            || compile_result
                .component_css
                .contains("container-name:card-header")
            || compile_result
                .component_css
                .contains("container-name: card-header")
    );
    assert!(
        compile_result.component_css.contains("card-header")
            && (compile_result.component_css.contains("640px")
                || compile_result.component_css.contains("min-width")
                || compile_result.component_css.contains("width>=640px"))
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
