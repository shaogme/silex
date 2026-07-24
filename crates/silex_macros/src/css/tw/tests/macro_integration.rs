use crate::css::tw::tw_impl;
use quote::quote;

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
fn test_conditional_tw_macro_deduplication() {
    let ts = quote!("p-4", (is_active, "bg-red-500", "bg-red-500"));
    let output = tw_impl(ts).unwrap();
    let code = output.to_string();
    // 每个编译块的 generate_inits 生成 2 个 inject_style 调用 (static_css 和 component_css)
    // 2 个不重复的规则块 (p-4 和 bg-red-500) 对应 4 个 inject_style 调用
    let inject_count = code.matches("inject_style").count();
    assert_eq!(
        inject_count, 4,
        "Expected exactly 4 inject_style calls (2 per unique CSS block for p-4 and bg-red-500), got {}",
        inject_count
    );
}
