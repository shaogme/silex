//! 颜色解析的宏侧入口——只做 `UtilityValue` 转换，规则全部来自 `silex_tw_core`。

use proc_macro2::Span;
use silex_tw_core::resolve_color_utility;
use syn::Result;

use super::core_bridge::{MacroCtx, rule_sets_to_rules};
use crate::css::tw::ast::{SpannedModifier, UtilityRule};

/// 解析颜色型 Utility 类（`text-slate-900`、`bg-indigo-600/50`、`ring-blue-500`、`divide-red-500` …）
///
/// 前缀表、伴生声明（ring 的 `box-shadow`、渐变的 `--tw-gradient-stops`）与伴生选择器
/// （`divide-*`、`placeholder-*`）都由 core 描述，这里只负责挂上修饰符与 span。
pub fn resolve_color_rules(
    modifiers: &[SpannedModifier],
    token: &str,
    span: Span,
) -> Option<Result<Vec<UtilityRule>>> {
    let sets = resolve_color_utility(&MacroCtx, token)?;
    Some(rule_sets_to_rules(modifiers, sets, span))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::tw::ast::UtilityValue;

    /// 非标色阶插值要经由宏侧注入的静态色板生效
    #[test]
    fn interpolates_non_standard_shades_through_the_generated_palette() {
        assert_eq!(
            silex_tw_core::lookup_palette_color(&MacroCtx, "slate", "900").as_deref(),
            Some("oklch(20.8% 0.042 265.755)")
        );
        assert!(
            silex_tw_core::lookup_palette_color(&MacroCtx, "slate", "850")
                .unwrap()
                .starts_with("oklch(")
        );
        assert!(silex_tw_core::lookup_palette_color(&MacroCtx, "indigo", "25").is_some());
        assert!(silex_tw_core::lookup_palette_color(&MacroCtx, "red", "975").is_some());
    }

    #[test]
    fn parses_arbitrary_hex_with_opacity() {
        let val = |t: &str| silex_tw_core::parse_color_value(&MacroCtx, t).map(|v| v.into_owned());
        assert_eq!(
            val("[#fff]/50").as_deref(),
            Some("color-mix(in oklab, #fff 50%, transparent)")
        );
        assert_eq!(
            val("[#1e293b80]/50").as_deref(),
            Some("color-mix(in oklab, #1e293b80 50%, transparent)")
        );
    }

    #[test]
    fn resolves_color_prefixes_but_leaves_size_scales_alone() {
        let span = Span::call_site();

        let rules = resolve_color_rules(&[], "bg-indigo-600/50", span)
            .unwrap()
            .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "background-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral(
                "color-mix(in oklab, oklch(51.1% 0.262 276.966) 50%, transparent)".to_string()
            )
        );

        let rules = resolve_color_rules(&[], "border-t-red-500", span)
            .unwrap()
            .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-top-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::ArbitraryLiteral("oklch(63.7% 0.237 25.331)".to_string())
        );

        // 尺寸档位不能被颜色路径吃掉
        assert!(resolve_color_rules(&[], "ring-2", span).is_none());
        assert!(resolve_color_rules(&[], "text-2xl", span).is_none());
    }
}
