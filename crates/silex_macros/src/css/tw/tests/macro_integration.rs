use crate::css::tw::tests::css_probe::extract_declarations;
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
/// 静态规则被提升为独立 Class 时，自动解出 table.rs 预置的 Tailwind v4 var(--tw-duration, ...) 默认变量声明。
#[test]
fn unspecified_duration_transition_utilities_hoist_with_default_duration() {
    // 1. 测试静态 `transition-all` 在条件 Cluster 碰撞下的独立提升与默认 duration 属性
    let all_ts = quote!(
        "peer inline-flex h-3.5 w-6 shrink-0 cursor-pointer items-center rounded-full border border-transparent p-0 shadow-xs transition-all outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
        (is_checked, "bg-primary", "bg-input dark:bg-input/80")
    );
    let all_css = css_of_tw(all_ts);
    assert!(!all_css.is_empty());
    let decls = extract_declarations(&all_css[0]);
    let prop_decl = decls
        .iter()
        .find(|(p, _)| p == "transition-property")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        prop_decl,
        Some("all"),
        "transition-all 提升必须精准产生 transition-property: all，实际声明: {decls:?}"
    );

    let dur_decl = decls
        .iter()
        .find(|(p, _)| p == "transition-duration")
        .map(|(_, v)| v.as_str());
    assert!(
        dur_decl == Some("var(--tw-duration,var(--default-transition-duration))")
            || dur_decl == Some("var(--tw-duration, var(--default-transition-duration))"),
        "transition-all 必须产出默认 var(--tw-duration, ...) 声明，实际声明: {decls:?}"
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
    let transform_decls = extract_declarations(&transform_css[0]);
    let transform_prop = transform_decls
        .iter()
        .find(|(p, _)| p == "transition-property")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        transform_prop,
        Some("transform,translate,scale,rotate").or(Some("transform, translate, scale, rotate")),
        "transition-transform 必须精准产生动画展开属性，实际声明: {transform_decls:?}"
    );
    let transform_dur = transform_decls
        .iter()
        .find(|(p, _)| p == "transition-duration")
        .map(|(_, v)| v.as_str());
    assert!(
        transform_dur == Some("var(--tw-duration,var(--default-transition-duration))")
            || transform_dur == Some("var(--tw-duration, var(--default-transition-duration))"),
        "transition-transform 必须产出默认 var(--tw-duration, ...) 声明，实际声明: {transform_decls:?}"
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
    let decls = extract_declarations(&colors_css[0]);
    let dur_decl = decls
        .iter()
        .find(|(p, _)| p == "transition-duration")
        .map(|(_, v)| v.as_str());
    assert!(
        dur_decl == Some("200ms") || dur_decl == Some(".2s"),
        "必须精准包含 200ms / .2s duration，实际声明: {decls:?}"
    );
    let ease_decl = decls
        .iter()
        .find(|(p, _)| p == "transition-timing-function")
        .map(|(_, v)| v.as_str());
    assert!(
        ease_decl == Some("cubic-bezier(.4,0,.2,1)")
            || ease_decl == Some("cubic-bezier(0.4, 0, 0.2, 1)"),
        "ease-in-out 必须精准解析为 cubic-bezier(0.4, 0, 0.2, 1)，实际声明: {decls:?}"
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
    let transform_decls = extract_declarations(&transform_css[0]);
    let transform_dur = transform_decls
        .iter()
        .find(|(p, _)| p == "transition-duration")
        .map(|(_, v)| v.as_str());
    assert!(
        transform_dur == Some("200ms") || transform_dur == Some(".2s"),
        "必须精准包含 200ms / .2s duration，实际声明: {transform_decls:?}"
    );
}

/// 验证 `transition-discrete` 与 `transition-normal` 独立提升出正确的 `transition-behavior` 规则
#[test]
fn transition_discrete_and_normal_utilities_hoist_correct_behavior_declarations() {
    // 1. 测试 transition-discrete 提升 allow-discrete
    let discrete_css = css_of_tw(quote!(
        "relative transition-discrete overflow-hidden",
        (is_visible, "block", "hidden")
    ));
    assert!(!discrete_css.is_empty());
    let discrete_decls = extract_declarations(&discrete_css[0]);
    let behavior_decl = discrete_decls
        .iter()
        .find(|(p, _)| p == "transition-behavior")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        behavior_decl,
        Some("allow-discrete"),
        "transition-discrete 必须精准产生声明 transition-behavior: allow-discrete，实际声明: {discrete_decls:?}"
    );

    // 2. 测试 transition-normal 提升 normal
    let normal_css = css_of_tw(quote!(
        "relative transition-normal overflow-hidden",
        (is_visible, "block", "hidden")
    ));
    assert!(!normal_css.is_empty());
    let normal_decls = extract_declarations(&normal_css[0]);
    let normal_behavior = normal_decls
        .iter()
        .find(|(p, _)| p == "transition-behavior")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        normal_behavior,
        Some("normal"),
        "transition-normal 必须精准产生声明 transition-behavior: normal，实际声明: {normal_decls:?}"
    );
}

/// 验证 `transition` 产出的完整展开 CSS 属性全集与 Tailwind v4 默认变量表达式
#[test]
fn full_transition_expanded_properties_and_variables_hoisting() {
    let css = css_of_tw(quote!(
        "transition flex items-center p-4",
        (active, "bg-blue-500", "bg-transparent")
    ));
    assert!(!css.is_empty());
    let decls = extract_declarations(&css[0]);

    let prop_decl = decls
        .iter()
        .find(|(p, _)| p == "transition-property")
        .map(|(_, v)| v.as_str());
    assert!(
        prop_decl.is_some(),
        "必须产生 transition-property 属性，实际声明: {decls:?}"
    );
    let prop_val = prop_decl.unwrap();

    // 严格校验 transition-property 是否精准包含了 Tailind v4 要求的全集展开项（包含渐变/现代属性）
    let expected_props = [
        "color",
        "background-color",
        "border-color",
        "outline-color",
        "text-decoration-color",
        "fill",
        "stroke",
        "--tw-gradient-from",
        "--tw-gradient-via",
        "--tw-gradient-to",
        "opacity",
        "box-shadow",
        "transform",
        "translate",
        "scale",
        "rotate",
        "filter",
        "-webkit-backdrop-filter",
        "backdrop-filter",
        "display",
        "content-visibility",
        "overlay",
        "pointer-events",
    ];
    let actual_props: Vec<&str> = prop_val.split(',').map(|s| s.trim()).collect();
    for expected in expected_props {
        assert!(
            actual_props.contains(&expected),
            "transition-property 必须精准包含子属性 '{expected}'，实际列表: {actual_props:?}"
        );
    }

    // 严格校验 transition-timing-function 与 transition-duration 的变量表达式
    let timing_decl = decls
        .iter()
        .find(|(p, _)| p == "transition-timing-function")
        .map(|(_, v)| v.as_str());
    assert!(
        timing_decl == Some("var(--tw-ease,var(--default-transition-timing-function))")
            || timing_decl == Some("var(--tw-ease, var(--default-transition-timing-function))"),
        "必须精准产生 var(--tw-ease...) 声明，实际声明: {decls:?}"
    );

    let dur_decl = decls
        .iter()
        .find(|(p, _)| p == "transition-duration")
        .map(|(_, v)| v.as_str());
    assert!(
        dur_decl == Some("var(--tw-duration,var(--default-transition-duration))")
            || dur_decl == Some("var(--tw-duration, var(--default-transition-duration))"),
        "必须精准产生 var(--tw-duration...) 声明，实际声明: {decls:?}"
    );
}

/// 验证包含 `transition-discrete`, `duration-300`, `delay-100`, `ease-out` 的复合动画控制规则提升与条件 Cluster 的隔离
#[test]
fn transition_control_rules_isolation_in_conditional_clusters() {
    let css = css_of_tw(quote!(
        "transition-all transition-discrete duration-300 delay-100 ease-out",
        (is_open, "opacity-100 scale-100", "opacity-0 scale-95")
    ));
    assert!(!css.is_empty());
    assert_eq!(
        css.len(),
        3,
        "静态提升段 1 个 + 2 个条件分支组 = 3 个 CSS 规则组"
    );

    // 1. 严格提取并校验第 0 组（静态提升的纯动画控制属性）
    let hoisted_decls = extract_declarations(&css[0]);
    let behavior = hoisted_decls
        .iter()
        .find(|(p, _)| p == "transition-behavior")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        behavior,
        Some("allow-discrete"),
        "静态提升段必须包含 transition-behavior: allow-discrete"
    );

    let transition_val = hoisted_decls
        .iter()
        .find(|(p, _)| p == "transition" || p == "transition-property")
        .map(|(_, v)| v.as_str());
    assert!(
        transition_val.is_some(),
        "必须产生 transition 简写或 longhand 声明，实际: {hoisted_decls:?}"
    );

    // 2. 严格提取并校验第 1 组（then 条件分支 opacity-100 scale-100）
    let cond1_decls = extract_declarations(&css[1]);
    assert_eq!(
        cond1_decls,
        vec![
            ("opacity".to_string(), "1".to_string()),
            ("scale".to_string(), "1".to_string())
        ],
        "then 分支只能精准包含 opacity: 1 和 scale: 1 声明"
    );

    // 3. 严格提取并校验第 2 组（else 条件分支 opacity-0 scale-95）
    let cond2_decls = extract_declarations(&css[2]);
    assert_eq!(
        cond2_decls,
        vec![
            ("opacity".to_string(), "0".to_string()),
            ("scale".to_string(), ".95".to_string())
        ],
        "else 分支只能精准包含 opacity: 0 和 scale: .95 声明"
    );
}

/// 验证 `mask` 渐变与方向工具类精准产出 `mask-composite: intersect`
#[test]
fn mask_utilities_emit_mask_composite() {
    // 1. 验证静态 mask 渐变工具类 (如 mask-circle)
    let css_circle = css_of_tw(quote!("mask-circle"));
    assert_eq!(css_circle.len(), 1);
    let decls_circle = extract_declarations(&css_circle[0]);
    let composite_circle = decls_circle
        .iter()
        .find(|(p, _)| p == "mask-composite")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        composite_circle,
        Some("intersect"),
        "mask-circle 必须包含 mask-composite: intersect，实际: {decls_circle:?}"
    );

    // 2. 验证动态 mask 角度工具类 (如 -mask-conic-0)
    let css_conic = css_of_tw(quote!("-mask-conic-0"));
    assert_eq!(css_conic.len(), 1);
    let decls_conic = extract_declarations(&css_conic[0]);
    let composite_conic = decls_conic
        .iter()
        .find(|(p, _)| p == "mask-composite")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        composite_conic,
        Some("intersect"),
        "-mask-conic-0 必须包含 mask-composite: intersect，实际: {decls_conic:?}"
    );
    let image_conic = decls_conic
        .iter()
        .find(|(p, _)| p == "mask-image")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        image_conic,
        Some("conic-gradient(from 0deg, var(--tw-mask-stops))"),
        "-mask-conic-0 必须格式化为 0deg，实际: {decls_conic:?}"
    );

    // 3. 验证 mask 颜色与 stop 方向工具类 (如 mask-l-to-yellow-600)
    let css_color = css_of_tw(quote!("mask-l-to-yellow-600"));
    assert_eq!(css_color.len(), 1);
    let decls_color = extract_declarations(&css_color[0]);
    let composite_color = decls_color
        .iter()
        .find(|(p, _)| p == "mask-composite")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        composite_color,
        Some("intersect"),
        "mask-l-to-yellow-600 必须包含 mask-composite: intersect，实际: {decls_color:?}"
    );
}
