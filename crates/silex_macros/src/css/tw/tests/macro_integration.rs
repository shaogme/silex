use crate::css::tw::tw_impl;
use proc_macro2::{TokenStream, TokenTree};
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

// ---------------------------------------------------------------------------
// §3.5 跨段层叠顺序
// ---------------------------------------------------------------------------

/// 收集宏展开里所有被注入的组件 CSS 片段
fn injected_css(ts: TokenStream) -> Vec<String> {
    fn walk(ts: TokenStream, out: &mut Vec<String>) {
        for tt in ts {
            match tt {
                TokenTree::Group(g) => walk(g.stream(), out),
                TokenTree::Literal(lit) => {
                    if let Ok(syn::Lit::Str(s)) = syn::parse_str::<syn::Lit>(&lit.to_string()) {
                        let v = s.value();
                        if v.starts_with("@layer utilities") {
                            out.push(v);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(ts, &mut out);
    // `generate_inits` 把同一段 CSS 写两次（`!css.is_empty()` 的判空 + 真正的调用）；
    // 不同组合归约到同一份产物时也会重复（`p-1 p-2 p-5` 与 `p-1 p-5` 都只剩 `p-5`）
    let mut seen = std::collections::HashSet::new();
    out.retain(|c| seen.insert(c.clone()));
    out
}

fn css_of_tw(ts: TokenStream) -> Vec<String> {
    injected_css(tw_impl(ts).unwrap())
}

/// 把整段词条写成一个静态字符串时的（唯一）产物
fn css_of_static(src: &str) -> String {
    let css = css_of_tw(quote!(#src));
    assert_eq!(css.len(), 1, "`{src}` 应该只产出一个类:\n{css:#?}");
    css.into_iter().next().unwrap()
}

/// 报告 §3.5：段与段之间没有编译期 tw-merge，谁覆盖谁取决于样式表注入顺序
/// （= 首次渲染顺序）。修复后，每个条件组合的产物必须与"把这些词条写在同一个
/// 字符串里"**逐字节一致**——类名是 CssBlock 的哈希，因此连类名都必须相同。
#[test]
fn conditional_branches_merge_with_the_static_segment() {
    let css = css_of_tw(quote!("p-4 md:p-6", (compact, "p-2")));

    // compact = true：`p-2` 覆盖基础的 `p-4`，但 `md:p-6` 依旧在 md 断点胜出——
    // 那是修饰符权重决定的，不是书写顺序。分处两个类时这一条根本无从表达。
    let merged = css_of_static("p-4 md:p-6 p-2");
    assert!(
        css.contains(&merged),
        "条件为真的组合应与 `p-4 md:p-6 p-2` 完全一致\n期望: {merged}\n实际: {css:#?}"
    );

    // compact = false：等价于只有静态段
    let plain = css_of_static("p-4 md:p-6");
    assert!(
        css.contains(&plain),
        "条件为假的组合应等价于静态段:\n{css:#?}"
    );

    assert_eq!(css.len(), 2, "两个组合、两个类，不该有多余产物:\n{css:#?}");
}

/// 简写/长写的覆盖关系同样要在编译期消解：`px-4` + `p-8` 合并后只剩 `padding`
#[test]
fn shorthand_longhand_conflicts_collapse_across_segments() {
    let css = css_of_tw(quote!("px-4", (big, "p-8")));
    assert!(css.contains(&css_of_static("px-4 p-8")), "{css:#?}");
    assert!(css.contains(&css_of_static("px-4")), "{css:#?}");
}

/// 多个条件分支落在同一簇时按 2^k 展开，每个组合都要与对应的静态写法一致
#[test]
fn multiple_conflicting_conditionals_expand_to_every_combination() {
    let css = css_of_tw(quote!("p-1", (a, "px-2"), (b, "py-3")));
    for expected in ["p-1", "p-1 px-2", "p-1 py-3", "p-1 px-2 py-3"] {
        assert!(
            css.contains(&css_of_static(expected)),
            "缺少 `{expected}` 对应的组合:\n{css:#?}"
        );
    }
    assert_eq!(css.len(), 4, "{css:#?}");
}

/// 不冲突的段**不合并**：类的数量与展开形态都与今天一致，不能因为这项修复而膨胀
#[test]
fn independent_segments_are_left_alone() {
    let out = tw_impl(quote!(
        "flex gap-2",
        (dim, "opacity-50"),
        (red, "text-red-500")
    ))
    .unwrap();
    let code = out.to_string();
    assert!(
        !code.contains("match"),
        "互不覆盖的段不该被展开成组合表:\n{code}"
    );
    // 静态段 1 个类 + 两个条件分支各 1 个类
    assert_eq!(injected_css(out).len(), 3);
}

/// 条件表达式只求值一次——它可能是读信号，重复求值既浪费也可能不等幂
#[test]
fn a_clustered_condition_is_evaluated_once() {
    let code = tw_impl(quote!("p-4", (is_big, "p-8"))).unwrap().to_string();
    assert_eq!(
        code.matches("is_big").count(),
        1,
        "条件应先绑定到局部变量再用于索引:\n{code}"
    );
}

/// 组合数按条件个数翻倍，超过上限时**报错**，而不是悄悄退回不确定的老行为
#[test]
fn too_many_conflicting_conditionals_are_rejected() {
    let err = tw_impl(quote!(
        "p-0",
        (a, "p-1"),
        (b, "p-2"),
        (c, "p-3"),
        (d, "p-4"),
        (e, "p-5"),
        (f, "p-6"),
        (g, "p-7")
    ))
    .unwrap_err()
    .to_string();
    assert!(err.contains("条件分支"), "{err}");
    assert!(err.contains("上限"), "{err}");
}

/// 验证未显式指定 duration/ease 的 transition 工具类（如 `transition-all` 与 `transition-transform`）
/// 静态规则被提升为独立 Class 时，自动解出 table.rs 预置的 150ms (.15s) 动画时长。
#[test]
fn unspecified_duration_transition_utilities_hoist_with_default_duration() {
    // 1. 测试静态 `transition-all` 在条件 Cluster 碰撞下的独立提升与默认 duration 属性
    let all_ts = quote!(
        "peer inline-flex h-3.5 w-6 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-all outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
        (is_checked, "bg-primary", "bg-input dark:bg-input/80")
    );
    let all_css = css_of_tw(all_ts);
    assert!(!all_css.is_empty());
    let all_hoisted_css = &all_css[0];
    assert!(
        all_hoisted_css.contains("transition-property:all")
            || all_hoisted_css.contains("transition-property: all"),
        "应该提升出 transition-property: all 静态类，实际: {all_hoisted_css}"
    );
    assert!(
        all_hoisted_css.contains("transition-duration:.15s")
            || all_hoisted_css.contains("transition-duration:150ms"),
        "底层由 codegen 产生的 table.rs 应为 transition-all 补全默认 .15s duration，实际: {all_hoisted_css}"
    );

    // 2. 测试静态 `transition-transform` 在条件 Cluster 碰撞下的独立提升与默认 duration 属性
    let transform_ts = quote!(
        "pointer-events-none block size-3 rounded-full bg-background ring-0 transition-transform",
        (
            is_checked,
            "translate-x-[calc(100%-2px)] dark:bg-primary-foreground",
            "translate-x-0 dark:bg-foreground"
        )
    );
    let transform_css = css_of_tw(transform_ts);
    assert!(!transform_css.is_empty());
    let transform_hoisted_css = &transform_css[0];
    assert!(
        transform_hoisted_css.contains("transition-property:transform"),
        "应该提升出 transition-property: transform 静态类，实际: {transform_hoisted_css}"
    );
    assert!(
        transform_hoisted_css.contains("transition-duration:.15s")
            || transform_hoisted_css.contains("transition-duration:150ms"),
        "底层由 codegen 产生的 table.rs 应为 transition-transform 补全默认 .15s duration，实际: {transform_hoisted_css}"
    );
}

/// 验证显式指定 duration-* 与 ease-* 的 transition 工具类（如 `transition-colors duration-200 ease-in-out`）
/// 静态规则被提升为独立 Class 时，正确解出自定义的动画时长与缓动函数。
#[test]
fn explicit_duration_and_ease_transition_utilities_hoist_with_custom_values() {
    // 1. 测试显式包含 duration-200 ease-in-out 的 `transition-colors` 静态类提升
    let colors_ts = quote!(
        "peer inline-flex h-3.5 w-6 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-colors duration-200 ease-in-out outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
        (is_checked, "bg-primary", "bg-input dark:bg-input/80")
    );
    let colors_css = css_of_tw(colors_ts);
    assert!(!colors_css.is_empty());
    let colors_hoisted_css = &colors_css[0];
    assert!(
        colors_hoisted_css.contains("transition-property:") && colors_hoisted_css.contains("color"),
        "应该提升出 transition-colors 规则，实际: {colors_hoisted_css}"
    );
    assert!(
        colors_hoisted_css.contains("transition-duration:.2s")
            || colors_hoisted_css.contains("transition-duration:200ms"),
        "包含显式 duration-200，应解析出 .2s duration，实际: {colors_hoisted_css}"
    );

    // 2. 测试显式包含 duration-200 ease-in-out 的 `transition-transform` 静态类提升
    let transform_ts = quote!(
        "pointer-events-none block size-3 rounded-full bg-background ring-0 transition-transform duration-200 ease-in-out",
        (
            is_checked,
            "translate-x-[calc(100%-2px)] dark:bg-primary-foreground",
            "translate-x-0 dark:bg-foreground"
        )
    );
    let transform_css = css_of_tw(transform_ts);
    assert!(!transform_css.is_empty());
    let transform_hoisted_css = &transform_css[0];
    assert!(
        transform_hoisted_css.contains("transition-property:transform"),
        "应该提升出 transition-property: transform 规则，实际: {transform_hoisted_css}"
    );
    assert!(
        transform_hoisted_css.contains("transition-duration:.2s")
            || transform_hoisted_css.contains("transition-duration:200ms"),
        "包含显式 duration-200，应解析出 .2s duration，实际: {transform_hoisted_css}"
    );
}
