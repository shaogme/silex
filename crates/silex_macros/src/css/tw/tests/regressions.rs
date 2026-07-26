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
    use crate::css::tw::ast::UtilityValue;
    use crate::css::tw::resolver::palette::parse_color_value;

    // `/1` = 1%，不是 100%
    assert_eq!(
        parse_color_value("red-500/1"),
        Some(UtilityValue::ArbitraryLiteral(
            "rgba(251, 44, 54, 0.01)".to_string()
        ))
    );
    assert_eq!(
        parse_color_value("red-500/50"),
        Some(UtilityValue::ArbitraryLiteral(
            "rgba(251, 44, 54, 0.5)".to_string()
        ))
    );
    // 小数不透明度走任意值语法 `/[0.5]`
    assert_eq!(
        parse_color_value("red-500/[0.5]"),
        Some(UtilityValue::ArbitraryLiteral(
            "rgba(251, 44, 54, 0.5)".to_string()
        ))
    );
}

#[test]
fn fractional_opacity_arbitrary_syntax_compiles() {
    assert_contains("text-red-500/[0.5]", "color:#fb2c3680");
}
