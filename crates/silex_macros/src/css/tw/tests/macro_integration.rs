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

/// 验证 `sr-only` 与 `not-sr-only` 产生现代 Web accessibility 规范中推荐的 `clip-path` 属性及全量属性集合。
#[test]
fn sr_only_utilities_emit_modern_clip_path() {
    // 1. 验证 `sr-only` 严格产出 9 个实体声明全集
    let css_sr = css_of_tw(quote!("sr-only"));
    assert_eq!(css_sr.len(), 1);
    let decls_sr = extract_declarations(&css_sr[0]);
    let decl_map_sr: std::collections::BTreeMap<&str, &str> = decls_sr
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();

    assert_eq!(decl_map_sr.get("clip-path"), Some(&"inset(50%)"));
    assert_eq!(decl_map_sr.get("position"), Some(&"absolute"));
    assert_eq!(decl_map_sr.get("width"), Some(&"1px"));
    assert_eq!(decl_map_sr.get("height"), Some(&"1px"));
    assert_eq!(decl_map_sr.get("padding"), Some(&"0"));
    assert_eq!(decl_map_sr.get("margin"), Some(&"-1px"));
    assert_eq!(decl_map_sr.get("overflow"), Some(&"hidden"));
    assert_eq!(decl_map_sr.get("white-space"), Some(&"nowrap"));
    assert_eq!(decl_map_sr.get("border-width"), Some(&"0"));
    assert!(
        !decl_map_sr.contains_key("clip"),
        "sr-only 严禁产出已被现代 CSS 标准废弃的 clip 属性"
    );

    // 2. 验证 `not-sr-only` 严格产出 8 个实体声明全集
    let css_not_sr = css_of_tw(quote!("not-sr-only"));
    assert_eq!(css_not_sr.len(), 1);
    let decls_not_sr = extract_declarations(&css_not_sr[0]);
    let decl_map_not_sr: std::collections::BTreeMap<&str, &str> = decls_not_sr
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();

    assert_eq!(decl_map_not_sr.get("clip-path"), Some(&"none"));
    assert_eq!(decl_map_not_sr.get("position"), Some(&"static"));
    assert_eq!(decl_map_not_sr.get("width"), Some(&"auto"));
    assert_eq!(decl_map_not_sr.get("height"), Some(&"auto"));
    assert_eq!(decl_map_not_sr.get("padding"), Some(&"0"));
    assert_eq!(decl_map_not_sr.get("margin"), Some(&"0"));
    assert_eq!(decl_map_not_sr.get("overflow"), Some(&"visible"));
    assert_eq!(decl_map_not_sr.get("white-space"), Some(&"normal"));
    assert!(
        !decl_map_not_sr.contains_key("clip"),
        "not-sr-only 严禁产出已被现代 CSS 标准废弃的 clip 属性"
    );
}

/// 验证 `sr-only` / `not-sr-only` 配合 Modifier (如 `focus:not-sr-only`) 与条件分支 (如 `tw!("sr-only", (is_focus, "not-sr-only"))`) 的正确展开与覆写。
#[test]
fn sr_only_utilities_with_modifiers_and_conditionals() {
    // 1. 验证 `focus:not-sr-only` 伪类变体正确绑定到 :focus 选择器且包含 clip-path: none
    let css_focus = css_of_tw(quote!("focus:not-sr-only"));
    assert_eq!(css_focus.len(), 1);
    assert!(
        css_focus[0].contains(":focus"),
        "focus:not-sr-only 必须产生包含 :focus 的选择器，实际: {}",
        css_focus[0]
    );
    let decls_focus = extract_declarations(&css_focus[0]);
    let decl_map_focus: std::collections::BTreeMap<&str, &str> = decls_focus
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(decl_map_focus.get("clip-path"), Some(&"none"));
    assert_eq!(decl_map_focus.get("position"), Some(&"static"));

    // 2. 验证真实无障碍 Skip Link 模式：`sr-only focus:not-sr-only` 在同一元素上无缝共存
    let css_skip_link = css_of_tw(quote!("sr-only focus:not-sr-only"));
    assert!(!css_skip_link.is_empty());
    let static_decls = extract_declarations(&css_skip_link[0]);
    let clip_sr = static_decls
        .iter()
        .find(|(p, _)| p == "clip-path")
        .map(|(_, v)| v.as_str());
    assert_eq!(
        clip_sr,
        Some("inset(50%)"),
        "基础状态依然为 sr-only (clip-path: inset(50%))"
    );

    // 3. 验证条件分支表达式中 `not-sr-only` 在条件为真时成功覆写 `sr-only`
    let css_cond = css_of_tw(quote!("sr-only", (is_focused, "not-sr-only")));
    assert_eq!(css_cond.len(), 2, "应该有两个条件组合的 CSS Class");

    // 找到包含 `not-sr-only` 覆写（组合为 "sr-only not-sr-only"）的真分支产出
    let true_branch_css = css_of_static("sr-only not-sr-only");
    assert!(
        css_cond.contains(&true_branch_css),
        "条件为真时 'sr-only not-sr-only' 合并后的 CSS 产物必须与静态写出的相同"
    );
    let decls_merged = extract_declarations(&true_branch_css);
    let decl_map_merged: std::collections::BTreeMap<&str, &str> = decls_merged
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        decl_map_merged.get("clip-path"),
        Some(&"none"),
        "tw-merge 后 not-sr-only 的 clip-path: none 应胜出"
    );
    assert_eq!(
        decl_map_merged.get("position"),
        Some(&"static"),
        "tw-merge 后 not-sr-only 的 position: static 应胜出"
    );
}

/// 验证 `font-mono` 工具类包含完整的跨平台回退字体栈（包含 Liberation Mono 与 Courier New）
#[test]
fn font_mono_utility_emits_full_fallback_stack() {
    let css = css_of_tw(quote!("font-mono"));
    assert!(!css.is_empty(), "font-mono 必须生成有效的 CSS 规则");

    let decls = extract_declarations(&css[0]);
    let font_family_decl = decls
        .iter()
        .find(|(p, _)| p == "font-family")
        .map(|(_, v)| v.as_str());

    assert_eq!(
        font_family_decl,
        Some(
            "ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,Liberation Mono,Courier New,monospace"
        ),
        "font-mono 产出的 font-family 必须包含完整的跨平台回退字体栈"
    );
}

/// 验证 `space-*` 与 `divide-*` 的编译期物理属性化与反转变量碰撞消解 (方案 A)
#[test]
fn space_and_divide_reversal_collision_collapses_in_compile_time() {
    // 1. 单独 space-x-4
    let css_space_x = css_of_static("space-x-4");
    let decls_space_x = extract_declarations(&css_space_x);
    let map_space_x: std::collections::BTreeMap<&str, &str> = decls_space_x
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(map_space_x.get("margin-left"), Some(&"1rem"));
    assert_eq!(map_space_x.get("margin-right"), Some(&"0"));

    // 2. 静态组合 space-x-4 space-x-reverse
    let css_space_x_rev = css_of_static("space-x-4 space-x-reverse");
    let decls_space_x_rev = extract_declarations(&css_space_x_rev);
    let map_space_x_rev: std::collections::BTreeMap<&str, &str> = decls_space_x_rev
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        map_space_x_rev.get("margin-left"),
        Some(&"0"),
        "space-x-reverse 必须将 margin-left 反转为 0"
    );
    assert_eq!(
        map_space_x_rev.get("margin-right"),
        Some(&"1rem"),
        "space-x-reverse 必须将 margin-right 反转为 1rem"
    );
    assert_eq!(
        map_space_x_rev.get("--tw-space-x-reverse"),
        None,
        "编译期物理消解不应残留 --tw-space-x-reverse 变量"
    );

    // 3. 静态组合 space-y-4 space-y-reverse
    let css_space_y_rev = css_of_static("space-y-4 space-y-reverse");
    let decls_space_y_rev = extract_declarations(&css_space_y_rev);
    let map_space_y_rev: std::collections::BTreeMap<&str, &str> = decls_space_y_rev
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(map_space_y_rev.get("margin-top"), Some(&"0"));
    assert_eq!(map_space_y_rev.get("margin-bottom"), Some(&"1rem"));

    // 4. 静态组合 divide-x-2 divide-x-reverse
    let css_divide_x_rev = css_of_static("divide-x-2 divide-x-reverse");
    let decls_divide_x_rev = extract_declarations(&css_divide_x_rev);
    let map_divide_x_rev: std::collections::BTreeMap<&str, &str> = decls_divide_x_rev
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(map_divide_x_rev.get("border-left-width"), Some(&"0"));
    assert_eq!(map_divide_x_rev.get("border-right-width"), Some(&"2px"));

    // 5. 跨段条件提升 Cluster 碰撞：tw!("space-x-4", cond && "space-x-reverse")
    let ts_cond = quote!("space-x-4", (reverse, "space-x-reverse"));
    let css_cond = css_of_tw(ts_cond);
    assert!(!css_cond.is_empty());
    let true_branch_css = css_of_static("space-x-4 space-x-reverse");
    assert!(
        css_cond.contains(&true_branch_css),
        "条件为真时 'space-x-4 space-x-reverse' Cluster 提升合并后的产出必须被编译期反转"
    );
}

/// 验证负间距（`-space-x-*` / `-space-y-*`）与 reverse 组合的物理方向反转测试
#[test]
fn space_negative_values_with_reverse_collision() {
    // 1. -space-x-4 space-x-reverse
    let css = css_of_static("-space-x-4 space-x-reverse");
    let decls = extract_declarations(&css);
    let map: std::collections::BTreeMap<&str, &str> = decls
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        map.get("margin-left"),
        Some(&"0"),
        "-space-x-4 经 reverse 物理互换后 margin-left 应为 0"
    );
    assert_eq!(
        map.get("margin-right"),
        Some(&"-1rem"),
        "-space-x-4 经 reverse 物理互换后 margin-right 应为 -1rem"
    );

    // 2. -space-y-6 space-y-reverse
    let css_y = css_of_static("-space-y-6 space-y-reverse");
    let decls_y = extract_declarations(&css_y);
    let map_y: std::collections::BTreeMap<&str, &str> = decls_y
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(map_y.get("margin-top"), Some(&"0"));
    assert_eq!(map_y.get("margin-bottom"), Some(&"-1.5rem"));
}

/// 验证带有修饰符（如媒体查询 `md:`, 伪类 `hover:`）的隔离性与翻转正确性
#[test]
fn space_and_divide_with_modifiers_and_breakpoints() {
    // 1. md:space-x-8 md:space-x-reverse
    let css_md = css_of_static("md:space-x-8 md:space-x-reverse");
    let decls_md = extract_declarations(&css_md);
    let map_md: std::collections::BTreeMap<&str, &str> = decls_md
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        map_md.get("margin-left"),
        Some(&"0"),
        "md: 下的 space-x-reverse 应将 margin-left 翻转为 0"
    );
    assert_eq!(
        map_md.get("margin-right"),
        Some(&"2rem"),
        "md: 下的 space-x-reverse 应将 margin-right 翻转为 2rem"
    );

    // 2. hover:divide-y-4 hover:divide-y-reverse
    let css_hover = css_of_static("hover:divide-y-4 hover:divide-y-reverse");
    let decls_hover = extract_declarations(&css_hover);
    let map_hover: std::collections::BTreeMap<&str, &str> = decls_hover
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(map_hover.get("border-top-width"), Some(&"0"));
    assert_eq!(map_hover.get("border-bottom-width"), Some(&"4px"));

    // 3. 修饰符隔离测试：hover:space-x-4 与无修饰符的 space-x-reverse
    // 无修饰符的 space-x-reverse 不该影响 hover:space-x-4 的 margin 属性
    let css_isolated = css_of_static("hover:space-x-4 space-x-reverse");
    let decls_isolated = extract_declarations(&css_isolated);
    let map_isolated: std::collections::BTreeMap<&str, &str> = decls_isolated
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        map_isolated.get("margin-left"),
        Some(&"1rem"),
        "hover: 上的 space-x-4 不会被无修饰符的 space-x-reverse 错杀反转"
    );
    assert_eq!(map_isolated.get("margin-right"), Some(&"0"));
}

/// 验证书写顺序无关性、双轴同时反转及复合 divide 工具类
#[test]
fn space_and_divide_dual_axis_and_order_invariance() {
    // 1. 反转修饰符在前的顺序无关性测试：space-x-reverse space-x-4
    let css_order = css_of_static("space-x-reverse space-x-4");
    let decls_order = extract_declarations(&css_order);
    let map_order: std::collections::BTreeMap<&str, &str> = decls_order
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        map_order.get("margin-left"),
        Some(&"0"),
        "space-x-reverse 放在前时仍需精准反转 margin-left 为 0"
    );
    assert_eq!(
        map_order.get("margin-right"),
        Some(&"1rem"),
        "space-x-reverse 放在前时仍需精准反转 margin-right 为 1rem"
    );

    // 2. 双轴同时反转：space-x-4 space-y-6 space-x-reverse space-y-reverse
    // LightningCSS 会自动将四边 margin 合并压缩为简写格式 `margin: top right bottom left`
    let css_dual = css_of_static("space-x-4 space-y-6 space-x-reverse space-y-reverse");
    let decls_dual = extract_declarations(&css_dual);
    let map_dual: std::collections::BTreeMap<&str, &str> = decls_dual
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        map_dual.get("margin"),
        Some(&"0 1rem 1.5rem 0"),
        "四边 margin 经反转消解后应被编译器精准压缩为 margin: 0 1rem 1.5rem 0"
    );

    // 3. 复合 divide 工具类（带线型与颜色）：divide-x-4 divide-x-reverse divide-dashed
    let css_divide_comp = css_of_static("divide-x-4 divide-x-reverse divide-dashed");
    let decls_divide_comp = extract_declarations(&css_divide_comp);
    let map_divide_comp: std::collections::BTreeMap<&str, &str> = decls_divide_comp
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    assert_eq!(map_divide_comp.get("border-left-width"), Some(&"0"));
    assert_eq!(map_divide_comp.get("border-right-width"), Some(&"4px"));
    assert_eq!(map_divide_comp.get("border-style"), Some(&"dashed"));
}

/// 验证 `text-*` 字号与 `leading-*` 行高在编译期的解耦消解机制（问题 6）
#[test]
fn text_and_leading_decoupling() {
    // Helper: 将 CSS 声明转化为 BTreeMap
    let to_map = |css: &str| {
        let decls = extract_declarations(css);
        let mut map = std::collections::BTreeMap::new();
        for (p, v) in decls {
            map.insert(p, v);
        }
        map
    };

    // 1. 顺序反转：leading-8 text-sm（显式 leading 在前，text-sm 在后）
    // 必须胜出：leading-8 的 2rem 压制 text-sm 自带的 1.25rem 默认行高
    let css_rev = css_of_static("leading-8 text-sm");
    let map_rev = to_map(&css_rev);
    assert_eq!(map_rev.get("font-size"), Some(&".875rem".to_string()));
    assert_eq!(map_rev.get("line-height"), Some(&"2rem".to_string()));

    // 2. 标准顺序：text-sm leading-8
    let css_std = css_of_static("text-sm leading-8");
    let map_std = to_map(&css_std);
    assert_eq!(map_std.get("font-size"), Some(&".875rem".to_string()));
    assert_eq!(map_std.get("line-height"), Some(&"2rem".to_string()));

    // 3. 无显式行高：text-sm text-lg（后写的 text-lg 全覆盖）
    let css_double_text = css_of_static("text-sm text-lg");
    let map_double_text = to_map(&css_double_text);
    assert_eq!(map_double_text.get("font-size"), Some(&"1.125rem".to_string()));
    assert_eq!(map_double_text.get("line-height"), Some(&"1.75rem".to_string()));

    // 4. 带斜杠简写：text-sm/6 leading-8（leading-8 在后）
    let css_slash_1 = css_of_static("text-sm/6 leading-8");
    let map_slash_1 = to_map(&css_slash_1);
    assert_eq!(map_slash_1.get("font-size"), Some(&".875rem".to_string()));
    assert_eq!(map_slash_1.get("line-height"), Some(&"2rem".to_string()));

    // 5. 带斜杠简写：leading-8 text-sm/6（text-sm/6 的 /6 在后）
    let css_slash_2 = css_of_static("leading-8 text-sm/6");
    let map_slash_2 = to_map(&css_slash_2);
    assert_eq!(map_slash_2.get("font-size"), Some(&".875rem".to_string()));
    assert_eq!(map_slash_2.get("line-height"), Some(&"1.5rem".to_string()));

    // 6. 多字号工具类与显式行高组合：leading-8 text-sm text-lg
    let css_multi = css_of_static("leading-8 text-sm text-lg");
    let map_multi = to_map(&css_multi);
    assert_eq!(map_multi.get("font-size"), Some(&"1.125rem".to_string()));
    assert_eq!(map_multi.get("line-height"), Some(&"2rem".to_string()));

    // 7. 修饰符隔离测试：hover:leading-8 text-sm
    let css_mod = css_of_static("hover:leading-8 text-sm");
    let decls_mod = extract_declarations(&css_mod);
    assert!(
        decls_mod.iter().any(|(p, v)| p == "font-size" && (v == ".875rem" || v == "0.875rem")),
        "基础修饰符应包含 text-sm 的 font-size: .875rem"
    );
    assert!(
        decls_mod.iter().any(|(p, v)| p == "line-height" && v == "1.25rem"),
        "无修饰符的基础组应保留 text-sm 自带的 1.25rem 默认行高"
    );
    assert!(
        decls_mod.iter().any(|(p, v)| p == "line-height" && v == "2rem"),
        "hover: 伪类下应独立包含 hover:leading-8 的 2rem 行高"
    );
}
