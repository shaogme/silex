//! Tailwind 工具类解析的**唯一真值源**。
//!
//! 这个 crate 存在的理由见分析报告 §3.1：此前 `silex_codegen`（离线生成静态表）与
//! `silex_macros`（编译期兜底解析模式型工具类）各自维护了一份逻辑独立的 resolver，
//! 二者对同一前缀的语义没有任何机制保证一致，已经发生过实际漂移
//! （`ring-<color>` 一侧映射到 `outline-color`、另一侧映射到 `--tw-ring-color`，
//! 且因为静态表优先命中，"正确"的那一侧成了永远不会执行的死代码）。
//!
//! 因此解析规则统一收敛到这里：
//!
//! - `silex_codegen` 调用本 crate 把类名批量求值成静态表（纯粹的**预计算缓存**）；
//! - `silex_macros` 先查那张静态表，未命中时调用本 crate 走同一套模式解析。
//!
//! 本 crate 刻意不依赖 `syn` / `proc-macro2`：产出是与宿主无关的
//! `(CSS 属性名, CSS 值)` 数据，span、`syn::Expr` 动态表达式等 proc-macro 概念
//! 由 `silex_macros` 侧的适配层负责附加。

#[macro_use]
mod macros;

pub mod at_rule;
pub mod color;
pub mod context;
pub mod kind;
pub mod palette;
pub mod prefix;
pub mod resolver;
pub mod value;

pub use at_rule::{AT_RULE_UTILITIES, AtRuleUtility, CONTAINER_TIERS, lookup_at_rule_utility};
pub use color::{
    apply_opacity, interpolate_oklch, lookup_palette_color, parse_color_value, parse_oklch,
    resolve_color_utility,
};
pub use context::TwContext;
pub use kind::{ValueKind, arbitrary_dispatch, classify_arbitrary_value};
pub use palette::{ColorShadeInfo, JsonPalette};
pub use prefix::{DIVIDE_SELECTOR, RING_BOX_SHADOW};
pub use resolver::resolve_class;
pub use value::{TwDecl, TwRuleSet, TwValueKind, classify, format_num, format_numeric};

/// 规范化 `tw_variants!` 的字符串选项键。
///
/// 过程宏的冲突检查与运行时字符串解析使用同一套规则，避免大小写、空白和
/// 分隔符差异造成多个声明映射到同一个选项。
pub fn normalize_variant_key(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::normalize_variant_key;

    #[test]
    fn variant_keys_ignore_case_whitespace_and_separators() {
        assert_eq!(normalize_variant_key(" Icon-Xs "), "iconxs");
        assert_eq!(normalize_variant_key("icon_xs"), "iconxs");
        assert_eq!(normalize_variant_key("SM"), "sm");
    }
}
