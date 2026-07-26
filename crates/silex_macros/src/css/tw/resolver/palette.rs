//! 颜色解析的宏侧入口——只做 `UtilityValue` 转换，规则全部来自 `silex_tw_core`。
//!
//! 这个模块此前有 ~290 行自己的色板/透明度/语义 token 实现，与 codegen 侧那份并行漂移
//! （报告 §3.1）。现在只剩转接：色阶插值、`/透明度`、`[#hex]`、语义 token、前缀映射
//! 一律走 core，两条路径共用同一份代码。

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

    /// 非标色阶插值要经由宏侧注入的静态色板生效（core 的单测用的是精简色板）
    #[test]
    fn interpolates_non_standard_shades_through_the_generated_palette() {
        assert_eq!(
            silex_tw_core::lookup_palette_color(&MacroCtx, "slate", "900").as_deref(),
            Some("#0f172b")
        );
        assert_eq!(
            silex_tw_core::lookup_palette_color(&MacroCtx, "slate", "850").as_deref(),
            Some("#162034")
        );
        assert!(silex_tw_core::lookup_palette_color(&MacroCtx, "indigo", "25").is_some());
        assert!(silex_tw_core::lookup_palette_color(&MacroCtx, "red", "975").is_some());
    }

    #[test]
    fn parses_arbitrary_hex_with_opacity() {
        let val = |t: &str| silex_tw_core::parse_color_value(&MacroCtx, t).map(|v| v.into_owned());
        assert_eq!(
            val("[#fff]/50").as_deref(),
            Some("rgba(255, 255, 255, 0.5)")
        );
        assert_eq!(
            val("[#1e293b80]/50").as_deref(),
            Some("rgba(30, 41, 59, 0.5)")
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
            UtilityValue::ArbitraryLiteral("rgba(79, 57, 246, 0.5)".to_string())
        );

        let rules = resolve_color_rules(&[], "border-t-red-500", span)
            .unwrap()
            .unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].css_property, "border-top-color");
        assert_eq!(
            rules[0].value,
            UtilityValue::HexColor("#fb2c36".to_string())
        );

        // 尺寸档位不能被颜色路径吃掉
        assert!(resolve_color_rules(&[], "ring-2", span).is_none());
        assert!(resolve_color_rules(&[], "text-2xl", span).is_none());
    }
}
