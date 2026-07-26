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
fn unsupported_functional_variant_reports_the_family() {
    let msg = err_of("max-md:flex");
    assert!(msg.contains("max-*"), "{msg}");
    assert!(msg.contains("not supported yet"), "{msg}");

    let msg = err_of("supports-[display:grid]:grid");
    assert!(msg.contains("supports-*"), "{msg}");
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
