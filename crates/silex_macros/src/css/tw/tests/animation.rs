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

fn static_css_of(src: &str) -> String {
    let input: TwInput = syn::parse2(quote!(#src)).unwrap();
    let css_block = build_css_block_from_tw(input).unwrap();
    crate::css::compiler::CssCompiler::compile(
        quote! { #css_block },
        proc_macro2::Span::call_site(),
        false,
    )
    .unwrap()
    .static_css
}

#[test]
fn test_animate_ping_pulse_bounce_css() {
    for (class, keyframe) in [
        ("animate-ping", "@keyframes ping"),
        ("animate-pulse", "@keyframes pulse"),
        ("animate-bounce", "@keyframes bounce"),
    ] {
        let css = static_css_of(class);
        assert!(
            css.contains(keyframe),
            "`{class}` 应注入 {keyframe}，实得:\n{css}"
        );
    }

    let bounce = static_css_of("animate-bounce");
    assert!(bounce.contains("cubic-bezier(.8,0,1,1)"));
    assert!(!bounce.contains("cubic - bezier"));
}

/// `animation` 是单一属性，同一组里写多个 `animate-*` 只有最后一个生效。
/// 被覆盖掉的那几个不能再把自己的 `@keyframes` 拖进产物——那是纯粹的死 CSS。
#[test]
fn overridden_animations_do_not_leak_their_keyframes() {
    let css = static_css_of("animate-ping animate-pulse animate-bounce");
    assert!(css.contains("@keyframes bounce"), "实得:\n{css}");
    assert!(!css.contains("@keyframes ping"), "实得:\n{css}");
    assert!(!css.contains("@keyframes pulse"), "实得:\n{css}");
}

/// 名字里**含有**内建动画名的自定义动画不得误命中。
/// 此前是在整条 `animation` 值上做 `contains("spin")`，
/// `animate-[spinner_2s_linear_infinite]` 于是白白注入一份 `@keyframes spin`。
#[test]
fn custom_animation_names_never_trigger_builtin_keyframes() {
    for class in [
        "animate-[spinner_2s_linear_infinite]",
        "animate-[my-ping_1s]",
        "animate-[pulsar_1s]",
        "animate-[bouncy_1s]",
    ] {
        let css = static_css_of(class);
        assert!(
            !css.contains("@keyframes"),
            "`{class}` 不该注入任何内建 keyframes，实得:\n{css}"
        );
    }
}

/// 不同修饰符组之间不会互相覆盖，两个动画的 keyframes 都要留下
#[test]
fn animations_in_distinct_modifier_groups_both_keep_their_keyframes() {
    let css = static_css_of("animate-spin hover:animate-pulse");
    assert!(css.contains("@keyframes spin"), "实得:\n{css}");
    assert!(css.contains("@keyframes pulse"), "实得:\n{css}");
}
