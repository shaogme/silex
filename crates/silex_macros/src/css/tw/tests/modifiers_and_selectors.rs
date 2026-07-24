use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use quote::quote;

#[test]
fn test_hover_blur_css() {
    let input: TwInput = syn::parse2(quote!("blur-sm hover:blur-none")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let block_ts = quote! { #css_block };
    let compile_result =
        crate::css::compiler::CssCompiler::compile(block_ts, proc_macro2::Span::call_site(), false)
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
    let input: TwInput = syn::parse2(quote!("group-hover:rotate-180 peer-focus:block")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let block_ts = quote! { #css_block };
    let compile_result =
        crate::css::compiler::CssCompiler::compile(block_ts, proc_macro2::Span::call_site(), false)
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
            || compile_result.component_css.contains(".peer:focus ~ .")
            || compile_result.component_css.contains(".peer:focus~."),
        "Expected peer-focus rule, got: {}",
        compile_result.component_css
    );
}

#[test]
fn test_named_group_and_data_attributes() {
    let input: TwInput = syn::parse2(quote!(
        "group/avatar group-data-[size=sm]/avatar:text-xs *:data-[slot=avatar]:ring-2"
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
            .contains(".group\\/avatar[data-size=sm]")
            || compile_result
                .component_css
                .contains(".group\\/avatar[data-size=\"sm\"]")
    );
    assert!(
        compile_result.component_css.contains("[data-slot=avatar]")
            || compile_result
                .component_css
                .contains("[data-slot=\"avatar\"]")
    );
}

#[test]
fn test_pseudo_class_child_selectors() {
    let input: TwInput = syn::parse2(quote!("border-b last:border-b-0 first:border-t-0")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let compile_result = crate::css::compiler::CssCompiler::compile(
        quote! { #css_block },
        proc_macro2::Span::call_site(),
        false,
    )
    .unwrap();
    assert!(compile_result.component_css.contains(":last-child"));
    assert!(compile_result.component_css.contains(":first-child"));
}

#[test]
fn test_origin_parenthesis_variable() {
    let input: TwInput =
        syn::parse2(quote!("origin-(--radix-popover-content-transform-origin)")).unwrap();
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
            .contains("transform-origin:var(--radix-popover-content-transform-origin)")
            || compile_result
                .component_css
                .contains("transform-origin: var(--radix-popover-content-transform-origin)")
    );
}
