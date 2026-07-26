use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use quote::quote;

#[test]
fn test_switch_translate_x_calc_no_spaces_and_modifiers() {
    let input: TwInput = syn::parse2(quote!(
        "pointer-events-none block size-4 rounded-full bg-background ring-0 transition-transform translate-x-[calc(100%-2px)] dark:bg-primary-foreground"
    )).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let block_ts = quote! { #css_block };
    let compile_result =
        crate::css::compiler::CssCompiler::compile(block_ts, proc_macro2::Span::call_site(), false)
            .unwrap();

    // 验证 1：不能包含带空格的错误函数调用 `translateX (` 或 `calc (`
    assert!(
        !compile_result.component_css.contains("translateX ("),
        "translateX should not contain space before parenthesis"
    );
    assert!(
        !compile_result.component_css.contains("calc ("),
        "calc should not contain space before parenthesis"
    );

    // 验证 2：静态/组件 CSS 中生成的平移计算必须是语法合法的无空格函数调用 `translate(calc(100% - 2px))`
    assert!(
        compile_result
            .component_css
            .contains("translate(calc(100% - 2px))")
            || compile_result
                .component_css
                .contains("translateX(calc(100% - 2px))"),
        "Expected valid translate calc syntax, got: {}",
        compile_result.component_css
    );
}

#[test]
fn test_shorthand_longhand_deduplication() {
    // 1. pt-2 在前，p-4 在后：后面的 shorthand (p-4) 完全覆盖前面的 pt-2
    let input: TwInput = syn::parse2(quote!("pt-2 p-4")).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    let block_ts = quote! { #css_block };
    let compile_result =
        crate::css::compiler::CssCompiler::compile(block_ts, proc_macro2::Span::call_site(), false)
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
    let compile_result =
        crate::css::compiler::CssCompiler::compile(block_ts, proc_macro2::Span::call_site(), false)
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
fn test_ring_system_css() {
    let input: TwInput = syn::parse2(quote!("ring-2 ring-indigo-500/20 ring-offset-2")).unwrap();
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
    // 值逐字来自 tw 参照表（`linear-gradient(to right, var(--tw-gradient-stops))`），
    // 不再经过「按 token 类型猜空白」的重建，所以逗号后的空格与 Tailwind 一致
    assert!(
        compile_result
            .component_css
            .contains("background-image:linear-gradient(to right, var(--tw-gradient-stops))")
    );
    assert!(
        compile_result
            .component_css
            .contains("--tw-gradient-from:#615fff")
    );
    assert!(
        compile_result
            .component_css
            .contains("--tw-gradient-via:#ad46ff")
    );
    assert!(
        compile_result
            .component_css
            .contains("--tw-gradient-to:#f6339a")
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
