//! 针对《TW 宏实现分析报告》第一阶段（止血）修复项的回归测试。
//!
//! 断言一律使用**精确**匹配子串（含空格与组合符），而不是宽松的 `contains("border")`
//! 之类的恒真断言——报告 §6.1 指出后者无法拦截任何一个 P0 缺陷。

use crate::css::tw::ast::TwInput;
use crate::css::tw::codegen::build_css_block_from_tw;
use quote::quote;

/// 编译一段 tw 词条为最终 CSS（component_css + static_css）
fn css_of(src: &str) -> String {
    let input: TwInput = syn::parse2(quote!(#src)).unwrap_or_else(|e| panic!("{src}: {e}"));
    let block = build_css_block_from_tw(input).unwrap_or_else(|e| panic!("{src}: {e}"));
    let r = crate::css::compiler::CssCompiler::compile(
        quote! { #block },
        proc_macro2::Span::call_site(),
        false,
    )
    .unwrap_or_else(|e| panic!("{src}: {e}"));
    format!("{}{}", r.component_css, r.static_css)
}

/// 编译一段 tw 词条，期望其失败并返回错误信息
fn err_of(src: &str) -> String {
    let parsed: syn::Result<TwInput> = syn::parse2(quote!(#src));
    match parsed {
        Err(e) => e.to_string(),
        Ok(input) => match build_css_block_from_tw(input) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected `{src}` to be rejected, but it compiled successfully"),
        },
    }
}

fn assert_contains(src: &str, needle: &str) {
    let css = css_of(src);
    assert!(
        css.contains(needle),
        "`{src}` should emit `{needle}`, got:\n{css}"
    );
}

// ---------------------------------------------------------------------------
// §2.1 group / peer 选择器必须带组合符
// ---------------------------------------------------------------------------

#[test]
fn group_variants_keep_descendant_combinator() {
    // 复合部分作用在 .group 自身，其后必须有后代组合符（空格）
    assert_contains(
        "group-data-[state=open]:text-red-500",
        ".group[data-state=open] .",
    );
    assert_contains("group-has-[.x]:text-red-500", ".group:has(.x) .");
    assert_contains("group-hover:text-red-500", ".group:hover .");
    assert_contains(
        "group-data-[size=sm]/avatar:text-red-500",
        ".group\\/avatar[data-size=sm] .",
    );
}

#[test]
fn peer_variants_keep_sibling_combinator() {
    let css = css_of("peer-data-[state=open]:text-red-500");
    assert!(
        css.contains(".peer[data-state=open]~.") || css.contains(".peer[data-state=open] ~ ."),
        "peer variant must keep the `~` combinator, got:\n{css}"
    );
}

#[test]
fn group_selector_never_produces_compound_with_target() {
    // 回归点：`.group[data-state=open].slx-tw-xxx` 是复合选择器，变体完全失效
    for src in [
        "group-data-[state=open]:text-red-500",
        "group-has-[.x]:text-red-500",
        "group-aria-[expanded=true]:text-red-500",
    ] {
        let css = css_of(src);
        assert!(
            !css.contains("].slx-tw") && !css.contains(").slx-tw"),
            "`{src}` produced a compound selector instead of a descendant one:\n{css}"
        );
    }
}

// ---------------------------------------------------------------------------
// §2.2 value_wrapper 必须被通用求值路径消费
// ---------------------------------------------------------------------------

#[test]
fn filter_family_wraps_values() {
    assert_contains("blur-[4px]", "filter:blur(4px)");
    assert_contains("blur-4", "filter:blur(4px)");
    assert_contains("backdrop-blur-[4px]", "backdrop-filter:blur(4px)");
    assert_contains("brightness-[1.75]", "filter:brightness(1.75)");
    assert_contains("hue-rotate-90", "filter:hue-rotate(90deg)");
}

#[test]
fn filter_utilities_never_emit_bare_values() {
    for src in [
        "blur-4",
        "blur-[4px]",
        "backdrop-blur-[4px]",
        "saturate-150",
    ] {
        let css = css_of(src);
        assert!(
            !css.contains("filter:4px") && !css.contains("filter:1.5"),
            "`{src}` emitted a bare value into a composite property:\n{css}"
        );
    }
}

#[test]
fn composable_properties_merge_within_a_modifier_group() {
    // filter / backdrop-filter / transform 的多条声明应拼接而非互相覆盖
    // （LightningCSS 会压掉函数之间的空格）
    assert_contains("blur-4 brightness-50", "filter:blur(4px)brightness(.5)");
    assert_contains(
        "translate-x-[2px] translate-y-[3px]",
        "transform:translate(2px)translateY(3px)",
    );
}

// ---------------------------------------------------------------------------
// §2.3 prefix_metadata 不得被 @property 描述符污染
// ---------------------------------------------------------------------------

#[test]
fn logical_border_arbitrary_value_is_clean() {
    let css = css_of("border-s-[3px]");
    assert!(
        css.contains("border-inline-start-width:3px"),
        "expected border-inline-start-width, got:\n{css}"
    );
    for junk in ["syntax:", "inherits:", "initial-value:", "-style:3px"] {
        assert!(
            !css.contains(junk),
            "`border-s-[3px]` leaked `{junk}` into the output:\n{css}"
        );
    }
}

#[test]
fn prefix_metadata_table_has_no_descriptor_props() {
    use crate::css::tw::resolver::codegen::prefix_metadata::PREFIX_METADATA;
    for meta in PREFIX_METADATA {
        for prop in meta.target_props {
            assert!(
                !matches!(*prop, "syntax" | "inherits" | "initial-value"),
                "PREFIX_METADATA still contains the at-rule descriptor `{prop}`"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// §2.4 ring 颜色统一走 --tw-ring-color
// ---------------------------------------------------------------------------

#[test]
fn ring_color_feeds_the_ring_variable() {
    let css = css_of("ring-2 ring-blue-500");
    assert!(
        css.contains("--tw-ring-color:#2b7fff"),
        "ring color must land on --tw-ring-color, got:\n{css}"
    );
    assert!(
        css.contains("--tw-ring-width:2px"),
        "ring width missing, got:\n{css}"
    );
    assert!(
        !css.contains("outline-color"),
        "ring color must not leak into outline-color:\n{css}"
    );
}

// ---------------------------------------------------------------------------
// §2.5 未知变体前缀必须报错，不得静默降级为伪类
// ---------------------------------------------------------------------------

#[test]
fn unknown_variant_prefix_is_rejected_with_suggestion() {
    let msg = err_of("mdd:flex");
    assert!(msg.contains("Unknown variant prefix 'mdd:'"), "{msg}");
    assert!(msg.contains("Did you mean 'md:'"), "{msg}");
}

#[test]
fn malformed_functional_variants_report_what_is_wrong() {
    // 函数式变体现已支持（§13.1），但**写坏了**仍必须报错而不是静默产出垃圾选择器
    let msg = err_of("supports-grid:flex");
    assert!(msg.contains("Write the feature query in brackets"), "{msg}");

    let msg = err_of("max-notabreakpoint:flex");
    assert!(msg.contains("unknown breakpoint"), "{msg}");

    let msg = err_of("nth-abc:flex");
    assert!(msg.contains("invalid"), "{msg}");

    // 伪元素没有"非此状态"的说法
    let msg = err_of("not-before:flex");
    assert!(msg.contains("cannot be negated"), "{msg}");

    // `in-*` 需要一个能选中元素的变体，媒体查询没有"祖先"可言
    let msg = err_of("in-print:flex");
    assert!(msg.contains("needs a variant that"), "{msg}");
}

#[test]
fn media_feature_variants_emit_media_queries() {
    assert_contains(
        "motion-reduce:flex",
        "@media (prefers-reduced-motion:reduce)",
    );
    assert_contains("print:hidden", "@media print");
    assert_contains("forced-colors:flex", "@media (forced-colors:active)");
    assert_contains("portrait:flex", "@media (orientation:portrait)");
}

#[test]
fn selector_variants_keep_their_full_selector() {
    // `[dir="rtl"] *` 中的后代组合符必须保留（走 TokenStream 会被吃掉空格）
    assert_contains("rtl:ml-4", "[dir=rtl] *");
    assert_contains("ltr:ml-4", "[dir=ltr] *");
}

#[test]
fn arbitrary_pseudo_class_passthrough_still_works() {
    assert_contains("[&:my-pseudo]:flex", ":my-pseudo");
}

// ---------------------------------------------------------------------------
// §2.6 /<number> 一律按百分比
// ---------------------------------------------------------------------------

#[test]
fn opacity_suffix_is_always_a_percentage() {
    // `/1` = 1%，不是 100%。断言编译产物而不是内部函数，
    // 这样无论解析走静态表还是模式兜底都能拦住。
    assert_contains("bg-red-500/1", "background-color:#fb2c3603");
    assert_contains("bg-red-500/50", "background-color:#fb2c3680");
    // 小数不透明度走任意值语法 `/[0.5]`
    assert_contains("bg-red-500/[0.5]", "background-color:#fb2c3680");
}

#[test]
fn fractional_opacity_arbitrary_syntax_compiles() {
    assert_contains("text-red-500/[0.5]", "color:#fb2c3680");
}

// ---------------------------------------------------------------------------
// §3.1 双 resolver 合并后修掉的漂移
// ---------------------------------------------------------------------------

/// Silex 把渐变方向内联进了 `linear-gradient(to right, var(--tw-gradient-stops))`，
/// 所以 `--tw-gradient-stops` 必须由色标工具类自己拼出来。
///
/// 合并前静态表里的 `from-*` 只写 `--tw-gradient-from`，宏兜底路径那份"会拼 stops"的
/// 实现因为静态表优先命中而成了死代码——结果 `bg-linear-to-r from-… to-…` 产出的
/// `linear-gradient(to right, )` 是无效声明，渐变整个不显示，且无任何报错。
#[test]
fn gradient_color_stops_define_the_stops_variable() {
    let css = css_of("bg-linear-to-r from-blue-500 to-pink-500");
    assert!(
        css.contains("--tw-gradient-stops:var(--tw-gradient-from), var(--tw-gradient-to)"),
        "渐变必须定义 --tw-gradient-stops，否则 background-image 整条失效，实得:\n{css}"
    );
    assert!(css.contains("--tw-gradient-from:#2b7fff"), "{css}");
    assert!(css.contains("--tw-gradient-to:#f6339a"), "{css}");

    assert_contains(
        "via-purple-500",
        "--tw-gradient-stops:var(--tw-gradient-from), var(--tw-gradient-via), var(--tw-gradient-to)",
    );
}

/// `placeholder-*` 的颜色落在 `::placeholder` 上，不是元素自身。
/// 合并前静态表没有承载伴生选择器的位置，291 个 `placeholder-*` 全都把颜色写在了元素上。
#[test]
fn placeholder_color_targets_the_placeholder_pseudo_element() {
    let css = css_of("placeholder-red-500");
    assert!(
        css.contains("::placeholder{color:#fb2c36}"),
        "placeholder-* 必须作用于 ::placeholder，实得:\n{css}"
    );
}

/// `divide-*` 的颜色落在相邻子元素之间。静态表与模式解析两条路径必须给出同一个选择器。
#[test]
fn divide_color_targets_adjacent_children() {
    let css = css_of("divide-red-500");
    assert!(
        css.contains(">:not([hidden])~:not([hidden]){border-color:#fb2c36}"),
        "divide-* 必须作用于相邻子元素，实得:\n{css}"
    );
}

// ---------------------------------------------------------------------------
// §2.8 多义前缀按值类型分派
// ---------------------------------------------------------------------------

/// 同一个前缀接长度和接颜色必须落到不同属性。
/// 此前靠"先查哪张表"的隐式顺序决定，只能有一种解释。
#[test]
fn ambiguous_prefixes_dispatch_on_the_value_kind() {
    // text-<长度> 是字号，不是颜色 —— 此前产出 `color:14px` 这种非法 CSS
    assert_contains("text-[14px]", "font-size:14px");
    assert_contains("text-[#818cf8]", "color:#818cf8");

    // 逻辑方向边框：长度走宽度，颜色走颜色
    assert_contains("border-s-[3px]", "border-inline-start-width:3px");
    assert_contains("border-s-[red]", "border-inline-start-color:red");
    assert_contains("border-[3px]", "border-width:3px");
    assert_contains("border-[red]", "border-color:red");

    // bg- 按值类型分派到 image / position / size
    assert_contains(
        "bg-[url(https://a.com/b.png)]",
        "background-image:url(https://a.com/b.png)",
    );
    assert_contains("bg-[#1e293b]", "background-color:#1e293b");

    // 非颜色、非尺寸的复合值仍应落到尺寸前缀的目标属性上
    assert_contains("shadow-[0_0_0_1px_red]", "box-shadow:0 0 0 1px red");
}

/// `ring-[…]` 的宽度形态要连带铺 box-shadow 载体，颜色形态走 `--tw-ring-color`
#[test]
fn ring_arbitrary_values_split_by_kind() {
    assert_contains("ring-[3px]", "--tw-ring-width:3px");
    assert_contains("ring-[rgba(79,70,229,.2)]", "--tw-ring-color:");
}

/// 宏兜底路径与静态表必须用同一份圆角档位表。
///
/// 第二阶段的对拍把 `rounded-*-sm` 从 v3 的 0.125rem 修正为 v4 的 0.25rem，
/// 但只改了 codegen 那一份；宏侧 `numeric.rs` 里的副本一直留着旧值，
/// 靠"静态表优先命中"才没暴露。现在两条路径共用 core 的实现。
#[test]
fn rounded_scale_has_a_single_source() {
    assert_contains("rounded-sm", "border-radius:.25rem");
    assert_contains("rounded-t-sm", "border-top-left-radius:.25rem");
    assert_contains("rounded-tl-sm", "border-top-left-radius:.25rem");
}

/// 颜色前缀表是唯一真值：这批前缀此前只有 codegen 侧认识，
/// macro 侧的 `ORDERED_PREFIXES` 只有 12 条，两侧覆盖范围不一致。
#[test]
fn extended_color_prefixes_resolve_to_their_own_properties() {
    assert_contains("text-shadow-red-500", "--tw-text-shadow-color:#fb2c36");
    assert_contains("shadow-red-500", "--tw-shadow-color:#fb2c36");
    assert_contains("decoration-red-500", "text-decoration-color:#fb2c36");
    assert_contains("inset-ring-red-500", "--tw-inset-ring-color:#fb2c36");
}

// ---------------------------------------------------------------------------
// §11.5 多义后缀的非任意值路径（路线图第 13 项）
// ---------------------------------------------------------------------------

/// `columns-<数字>` 是列数、`columns-<容器档位>` 是列宽，二者都写进 `columns` 简写。
///
/// 此前按后缀形态分派到 `column-count` / `column-width` 两个长写属性，
/// 而档位表是共用的：`columns-lg` 于是产出 `column-count:32rem` 这样的非法 CSS。
#[test]
fn columns_always_uses_the_shorthand() {
    assert_contains("columns-4", "columns:4");
    assert_contains("columns-lg", "columns:32rem");
    assert_contains("columns-auto", "columns:auto");
    // 静态表未覆盖的档位走宏兜底路径，结论必须一致
    assert_contains("columns-13", "columns:13");
    assert_contains("columns-9xl", "columns:128rem");

    for src in ["columns-4", "columns-lg", "columns-13"] {
        let css = css_of(src);
        assert!(
            !css.contains("column-count") && !css.contains("column-width"),
            "`{src}` should not emit the longhand properties, got:\n{css}"
        );
    }
}

/// `flex-<数字>` 是无单位的 flex 简写，不是间距档位。
///
/// `resolve_length_val` 此前先于数值判定命中，把 `flex-4` 求成 `flex:1 1 1rem`。
#[test]
fn numeric_flex_is_unitless() {
    assert_contains("flex-4", "flex:4");
    assert_contains("flex-1", "flex:1");
    // 静态表未覆盖的档位走宏兜底路径
    assert_contains("flex-13", "flex:13");
    // 分数形态仍是 flex-basis 百分比，不能被数值分支吞掉
    // （`1 1 50%` 被 LightningCSS 压成等价的 `50%`）
    assert_contains("flex-1/2", "flex:50%");

    let css = css_of("flex-4");
    assert!(
        !css.contains("rem"),
        "`flex-4` must not be evaluated on the spacing scale, got:\n{css}"
    );
}

// ---------------------------------------------------------------------------
// §3.2 divide / space 的伴生选择器（路线图第 12 项）
// ---------------------------------------------------------------------------

/// `divide-*` / `space-*` 的声明落在**相邻子元素之间**，绝不能落在元素自身。
///
/// 这一族此前有两份实现：宏侧一条带 `DIVIDE_SELECTOR` 的 80 行硬编码长链，
/// core 侧另有一份**不带选择器**的 `(属性, 值)` 映射。宏那份排在前面，
/// core 那份是永不命中的死代码——一旦顺序变动，边框就会画到容器自己身上。
#[test]
fn divide_and_space_declarations_land_between_children() {
    // LightningCSS 会压掉组合符两侧的空格
    let sep = ">:not([hidden])~:not([hidden])";
    for src in [
        "divide-x",
        "divide-y-4",
        "divide-dashed",
        "divide-x-reverse",
        "space-x-4",
        "-space-y-2",
        "space-y-reverse",
    ] {
        let css = css_of(src);
        assert!(
            css.contains(sep),
            "`{src}` must be scoped to the child separator, got:\n{css}"
        );
    }

    assert_contains("divide-x", "border-left-width:1px");
    assert_contains("divide-x", "border-right-width:0");
    assert_contains("divide-y-4", "border-top-width:4px");
    assert_contains("divide-dashed", "border-style:dashed");
    assert_contains("space-x-4", "margin-left:1rem");
    assert_contains("space-x-4", "margin-right:0");
    assert_contains("-space-y-2", "margin-top:-.5rem");
}

/// 伴生声明与取反都由 `prefix_metadata` 承载，不再是 `numeric.rs` 里的 `if prefix == …`。
///
/// 回归点：`outline-style: solid` 此前只写在数值分支里，任意值路径 `outline-[3px]`
/// 漏掉它——CSS 的 `outline-style` 默认是 `none`，漏掉就等于这条 outline 画不出来。
#[test]
fn prefix_companions_apply_to_both_fallback_paths() {
    // 静态表未覆盖的档位，走宏兜底
    assert_contains("outline-7", "outline-style:solid");
    assert_contains("outline-7", "outline-width:7px");
    assert_contains("outline-[3px]", "outline-style:solid");
    assert_contains("outline-[3px]", "outline-width:3px");
}

/// `slide-in-from-top/left` 的位移方向为负，由 `value_wrapper: "-{}"` 表达
#[test]
fn slide_in_direction_comes_from_the_value_wrapper() {
    assert_contains("slide-in-from-top-4", "--tw-enter-translate-y:-1rem");
    assert_contains("slide-in-from-left-4", "--tw-enter-translate-x:-1rem");
    assert_contains("slide-in-from-bottom-4", "--tw-enter-translate-y:1rem");
    assert_contains("slide-in-from-right-4", "--tw-enter-translate-x:1rem");
}

// ---------------------------------------------------------------------------
// §13.2 `!important`、负数任意值、字号/行高简写（第四阶段第 16 项）
// ---------------------------------------------------------------------------

#[test]
fn important_marker_is_accepted_in_both_positions() {
    // v4 的后置写法与 v3 的前置写法都支持
    assert_contains("p-4!", "padding:1rem!important");
    assert_contains("!p-4", "padding:1rem!important");
    // 变体前缀之后仍然识别
    assert_contains("hover:p-4!", "padding:1rem!important");
    assert_contains("hover:!p-4", "padding:1rem!important");
    // 任意值与颜色路径同样生效
    assert_contains("w-[3px]!", "width:3px!important");
    assert_contains("text-red-500!", "color:#fb2c36!important");
}

#[test]
fn important_participates_in_tw_merge_as_last_wins() {
    // 编译期 tw-merge 的语义始终是 last-wins，`!` 不改变谁赢——
    // 否则 `p-4! p-8` 会留下两条声明，反而回到运行时靠优先级打架
    let css = css_of("p-4! p-8");
    assert!(css.contains("padding:2rem"), "{css}");
    assert!(!css.contains("!important"), "{css}");
}

#[test]
fn negative_arbitrary_values_are_negated_not_rejected() {
    // 回归点：`-mt-[10px]` 此前拿 `-mt` 去查 CssPropertyId，报的是内部错误
    assert_contains("-mt-[10px]", "margin-top:-10px");
    assert_contains("-m-[10px]", "margin:-10px");
    // 取反在内层：`rotate(calc(45deg * -1))`，不是 `calc(rotate(45deg) * -1)`
    assert_contains("-rotate-[45deg]", "transform:rotate(-45deg)");
    // `var()` / `calc()` 只能靠 calc 取反，加个负号会产出非法值
    assert_contains("-w-[var(--x)]", "width:calc(var(--x) * -1)");
    // 值里自带负号的写法保持原样
    assert_contains("mt-[-10px]", "margin-top:-10px");
}

#[test]
fn arbitrary_value_with_unknown_prefix_reports_the_utility_not_the_property_table() {
    // 报告 §2.7：不得把 "CssPropertyId 表里没有 'foo'" 这种内部细节抛给用户
    let msg = err_of("foo-[3px]");
    assert!(msg.contains("Unknown utility prefix 'foo'"), "{msg}");
    assert!(!msg.contains("CssPropertyId"), "{msg}");
}

#[test]
fn font_size_slash_leading_shorthand() {
    assert_contains("text-[14px]/[1.5]", "font-size:14px");
    assert_contains("text-[14px]/[1.5]", "line-height:1.5");
    assert_contains("text-sm/6", "line-height:1.5rem");
    assert_contains("text-sm/loose", "line-height:2");
    // 字号档位自带的行高必须被显式写出的那个替换，而不是两条都留下
    let css = css_of("text-sm/6");
    assert_eq!(css.matches("line-height").count(), 1, "{css}");
    // 同形的颜色 + 不透明度不能被这条路径截走
    assert_contains("text-red-500/50", "color:#fb2c3680");
    let msg = err_of("text-sm/nope");
    assert!(msg.contains("Unknown line-height 'nope'"), "{msg}");
}

/// 组合型属性合并时**不能**把分量提前渲染成字符串
///
/// 回归点：合并逻辑曾对每条分量 `to_string()` 再拼接，`$(signal)` 于是被压成裸标识符
/// `signal`，`blur-[$(v)]` 产出 `filter: blur(v)`——CSS 里没有这个值，静默失效。
#[test]
fn composable_merge_keeps_dynamic_expressions_at_token_level() {
    let css = css_of("blur-[$(my_signal)]");
    assert!(css.contains("filter:blur(var(--"), "{css}");
    let css = css_of("blur-[$(a)] brightness-50");
    assert!(css.contains("blur(var(--"), "{css}");
    assert!(css.contains("brightness(.5)"), "{css}");
}

/// `value_wrapper` 必须同样作用在动态表达式上
#[test]
fn value_wrapper_applies_to_dynamic_expressions() {
    // 回归点：`build_value` 遇到 `$(…)` 时直接 return，把 wrapper 丢了
    let css = css_of("blur-[$(v)]");
    assert!(css.contains("blur("), "wrapper 丢失: {css}");
    let css = css_of("-mt-[$(v)]");
    assert!(css.contains("calc(var("), "取反 wrapper 丢失: {css}");
}

// ---------------------------------------------------------------------------
// §13.3 组合型属性跨修饰符组叠加（第四阶段第 19 项）
// ---------------------------------------------------------------------------

/// 回归点：`transform` 在 CSS 里是单一属性，`hover:` 那条会**整条**盖掉基础那条。
/// 修复前 `hover:translate-x-2 translate-y-2` 在 hover 时 Y 位移凭空消失。
#[test]
fn composable_properties_stack_across_modifier_groups() {
    let css = css_of("hover:translate-x-2 translate-y-2");
    // 基础组只有 Y
    assert!(css.contains("transform:translateY(.5rem)}"), "{css}");
    // hover 组必须同时带上继承来的 Y 与自己的 X
    let hover = css
        .split(":hover")
        .nth(1)
        .unwrap_or_else(|| panic!("产物里没有 hover 规则:\n{css}"));
    assert!(
        hover.contains("translateY(.5rem)") && hover.contains("translate(.5rem)"),
        "hover 组丢了基础组的分量:\n{css}"
    );

    // 书写顺序不影响结果
    assert_eq!(
        css_of("hover:translate-x-2 translate-y-2"),
        css_of("translate-y-2 hover:translate-x-2")
    );
}

#[test]
fn composable_inheritance_follows_the_subset_relation() {
    // `md:hover:` 生效时 `md:` 必然也生效，因此要继承 `md:` 的分量
    let css = css_of("md:translate-x-2 md:hover:translate-y-2");
    let hover = css.split(":hover").nth(1).unwrap();
    assert!(
        hover.contains("translate(.5rem)") && hover.contains("translateY(.5rem)"),
        "md:hover 应继承 md: 的分量:\n{css}"
    );

    // 同名函数仍然 last-wins，不是叠加两次
    let css = css_of("translate-x-2 hover:translate-x-4");
    let hover = css.split(":hover").nth(1).unwrap();
    assert!(hover.contains("translate(1rem)"), "{css}");
    assert!(!hover.contains(".5rem"), "同名函数不应叠加:\n{css}");
}

#[test]
fn composable_inheritance_never_invents_declarations() {
    // 本组没用到 transform 时不得凭空长出一条
    let css = css_of("hover:flex translate-y-2");
    let hover = css.split(":hover").nth(1).unwrap();
    assert!(
        !hover.contains("transform"),
        "hover:flex 不该凭空获得 transform:\n{css}"
    );
}

// ---------------------------------------------------------------------------
// §13.4 细粒度错误 Span（第四阶段第 17 项）
// ---------------------------------------------------------------------------

/// 报告 §4.1：错误此前把整个字符串字面量标红。
///
/// stable 上 `proc_macro::Literal::subspan` 仍未稳定（`proc_macro2` 恒返回 `None`），
/// 拿不到真正的子 span，于是退一步在错误信息里画插入符指出是哪个词条。
/// nightly 上 subspan 生效，rustc 的箭头直接指到位，这段上下文就不再附加。
#[test]
fn errors_point_at_the_offending_token_not_the_whole_string() {
    let msg = err_of("flex items-center p-44x rounded-lg");
    assert!(msg.contains("Did you mean 'p-44'?"), "{msg}");
    if msg.contains("in `tw!` string:") {
        // 插入符必须落在 `p-44x` 上：前面 18 个字符是 "flex items-center "
        let caret_line = msg.lines().last().unwrap();
        assert_eq!(
            caret_line.trim_end(),
            format!("    {}{}", " ".repeat(18), "^".repeat(5)),
            "插入符位置不对:\n{msg}"
        );
    }
}

#[test]
fn errors_on_a_variant_prefix_point_at_that_prefix_only() {
    let msg = err_of("md:hoveer:flex");
    assert!(msg.contains("Did you mean 'hover:'"), "{msg}");
    if msg.contains("in `tw!` string:") {
        // 只出现一次上下文：内层变体已经画得更精确，外层不该再叠一遍整词条的
        assert_eq!(msg.matches("in `tw!` string:").count(), 1, "{msg}");
        let caret_line = msg.lines().last().unwrap();
        assert_eq!(
            caret_line.trim_end(),
            format!("    {}{}", " ".repeat(3), "^".repeat(6)),
            "插入符应只覆盖 `hoveer`:\n{msg}"
        );
    }
}

#[test]
fn long_class_strings_are_windowed_around_the_error() {
    let long = "flex p-4 mt-2 mb-3 ml-4 mr-5 gap-2 rounded-lg shadow-md border text-sm font-bold p-44x uppercase";
    let msg = err_of(long);
    if msg.contains("in `tw!` string:") {
        assert!(msg.contains('…'), "超长字符串应被截断成窗口:\n{msg}");
        // 出错词条本身必须在窗口内
        assert!(msg.contains("p-44x"), "{msg}");
    }
}

// ---------------------------------------------------------------------------
// §13.7 抽取脚本漏列的工具类
// ---------------------------------------------------------------------------

/// `getClassList()` 是给编辑器补全用的，只覆盖"能从 theme 展开出取值"的部分。
/// 取值不来自 theme 的 `filter-none` / `backdrop-filter-none`，以及静态工具类里的
/// 一批 v3 兼容别名，整条不在里面——它们此前一律报"未知类"。
/// 抽取脚本改以 `designSystem.utilities.keys()`（Tailwind 自己的注册表）兜底。
#[test]
fn utilities_missing_from_get_class_list_now_resolve() {
    assert_contains("filter-none", "filter:none");
    assert_contains("backdrop-filter-none", "backdrop-filter:none");

    // v3 轴序写法的位置别名（LightningCSS 会把关键字压成等价的百分比）
    assert_contains("bg-left-top", "background-position:0 0");
    assert_contains("bg-right-bottom", "background-position:100% 100%");
    assert_contains("object-left-top", "object-position:top left");
    assert_contains("object-right-bottom", "object-position:bottom right");

    // 其余 v3 兼容别名
    assert_contains("overflow-ellipsis", "text-overflow:ellipsis");
    assert_contains("break-words", "overflow-wrap:break-word");
    assert_contains("decoration-slice", "box-decoration-break:slice");
    assert_contains("decoration-clone", "box-decoration-break:clone");
    assert_contains("order-none", "order:0");
}

/// `none` 是整条属性的关键字取值，不能与函数分量并列——
/// `filter: blur(4px) none` 是非法 CSS。组合型属性的合并逻辑必须让它清空此前累积的分量。
#[test]
fn filter_none_clears_previously_accumulated_filter_components() {
    assert_contains("blur-sm brightness-50 filter-none", "filter:none");
    let css = css_of("blur-sm brightness-50 filter-none");
    assert!(
        !css.contains("blur(") && !css.contains("brightness("),
        "filter-none 之后不该还留着函数分量:\n{css}"
    );

    // 反过来：filter-none 在前，后面的分量照常累积
    assert_contains("filter-none blur-sm", "filter:blur(4px)");
}

// ---------------------------------------------------------------------------
// §14.3 / §2.10 第六阶段收口
// ---------------------------------------------------------------------------

/// `inset-ring-*` 此前是三重静默失效：宽度落在 `outline-width` + `outline-offset`
/// 上（而且没有 `outline-style`，默认 `none`，画不出来）、颜色落在
/// `--tw-inset-ring-color` 上（没人消费）、任意值因为宽度分派表漏了这个前缀
/// 被判成颜色，产出 `--tw-inset-ring-color: 3px` 这种非法 CSS。
#[test]
fn inset_ring_width_and_color_share_one_box_shadow_carrier() {
    for (src, needle) in [
        ("inset-ring", "--tw-inset-ring-width:1px"),
        ("inset-ring-2", "--tw-inset-ring-width:2px"),
        // 静态表没有的档位，走 core 兜底
        ("inset-ring-7", "--tw-inset-ring-width:7px"),
        // 任意值：长度归宽度
        ("inset-ring-[3px]", "--tw-inset-ring-width:3px"),
        // 任意值：颜色归颜色
        ("inset-ring-[red]", "--tw-inset-ring-color:red"),
        ("inset-ring-red-500", "--tw-inset-ring-color:#fb2c36"),
    ] {
        assert_contains(src, needle);
        // 无论宽度还是颜色，都必须铺同一条 box-shadow 载体，否则写进变量的值没人读
        assert_contains(
            src,
            "var(--tw-inset-ring-width,0px) var(--tw-inset-ring-color",
        );
    }

    // 宽度 + 颜色一起写才是常见用法，两者必须落在同一条 box-shadow 上
    let css = css_of("inset-ring-2 inset-ring-red-500");
    assert!(
        css.contains("--tw-inset-ring-width:2px") && css.contains("--tw-inset-ring-color:#fb2c36"),
        "实得:\n{css}"
    );
    assert!(
        !css.contains("outline-width"),
        "inset-ring 不该再动 outline:\n{css}"
    );
}

/// v4 里 `outline-none` 是"没有描边"，`outline-hidden` 才是
/// "视觉上去掉、但强制配色模式下保留一条透明描边"。此前两者的语义是互换的，
/// 而且 `outline-hidden` 产出的是 `outline-style: hidden`。
#[test]
fn outline_none_and_outline_hidden_have_v4_semantics() {
    let none = css_of("outline-none");
    assert!(none.contains("outline-style:none"), "实得:\n{none}");
    assert!(
        !none.contains("2px solid"),
        "outline-none 不该再画透明描边:\n{none}"
    );

    let hidden = css_of("outline-hidden");
    assert!(hidden.contains("outline-style:none"), "实得:\n{hidden}");
    assert!(
        hidden.contains("forced-colors") && hidden.contains("2px solid"),
        "outline-hidden 的透明描边必须落在 forced-colors 里:\n{hidden}"
    );
}

/// `container` 与 `@container` 此前产出完全相同的 CSS。
/// Tailwind 里前者是"宽度撑满、到断点为止"的容器工具类，后者才是容器查询上下文。
#[test]
fn container_is_a_breakpoint_width_utility_not_a_container_query_context() {
    let css = css_of("container");
    assert!(css.contains("width:100%"), "实得:\n{css}");
    assert!(
        !css.contains("container-type"),
        "container 不是容器查询上下文:\n{css}"
    );
    for width in ["40rem", "48rem", "64rem", "80rem", "96rem"] {
        assert!(
            css.contains(&format!("max-width:{width}")),
            "缺 {width} 档位:\n{css}"
        );
    }

    // `@container` 不受影响
    assert_contains("@container", "container-type:inline-size");
}

/// marker class 的定义是"必须以字面类名出现在 DOM 上，好让别的选择器引用它"。
/// 容器查询的 `container-type` / `container-name` 都是声明，落在哈希类上就够了；
/// 此前 `@container` / `container/side` 这种连合法类名都不是的字符串会被塞进 class 属性。
#[test]
fn only_group_and_peer_land_in_the_class_attribute() {
    use crate::css::tw::ast::TwInput;

    let extra_of = |src: &str| -> Vec<String> {
        let input: TwInput = syn::parse2(quote!(#src)).unwrap();
        input.extra_classes
    };

    assert_eq!(extra_of("group p-4"), vec!["group".to_string()]);
    assert_eq!(extra_of("peer/x p-4"), vec!["peer/x".to_string()]);
    assert!(extra_of("@container @sm:p-4").is_empty());
    assert!(extra_of("@container/card p-4").is_empty());
    assert!(extra_of("container p-4").is_empty());
    assert!(extra_of("container/side p-4").is_empty());
}
