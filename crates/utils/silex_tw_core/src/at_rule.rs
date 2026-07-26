//! 产出**带 at-rule 分组**的工具类。
//!
//! 静态表的一行只能挂一个伴生选择器、挂不了 at-rule
//! （`silex_codegen` 的 `tables.rs::flatten` 对此有显式断言），
//! 所以这一族由宏层展开成多个修饰符组。
//!
//! 但规则本身仍然写在这里，而不是写进 `silex_macros`：报告 §3.1 反复出现的病理
//! 就是"同一语义存在两份实现，其中一份因为查表顺序永远不会执行"。
//! 这张表是唯一真值，宏层只有一个通用展开器，`silex_codegen` 读同一张表
//! 把这些类名从"未实现"名单里排除——两侧对"哪些类名归这条路径"的认知不可能分叉。

/// 一组共享同一 at-rule 条件的声明
#[derive(Debug, Clone, Copy)]
pub struct CondDecls {
    /// `None` 表示无条件；`Some(cond)` 表示落在 `@media cond` 里
    pub media: Option<&'static str>,
    pub decls: &'static [(&'static str, &'static str)],
}

/// 一个需要 at-rule 分组的工具类
#[derive(Debug, Clone, Copy)]
pub struct AtRuleUtility {
    pub class: &'static str,
    /// 固定的声明分组
    pub groups: &'static [CondDecls],
    /// 额外按断点展开：每个断点产出一条 `@media (width >= W) { <prop>: W }`
    pub per_breakpoint: Option<&'static str>,
}

/// `container` 的档位。
///
/// 刻意用 Tailwind 自己的 `--container-*` 值（rem）而不是 silex 断点表里的 px 写法：
/// 两者数值等价（40rem = 640px），但照抄 rem 能让产物与真实 Tailwind 逐字节一致，
/// 对拍测试因此不需要为 `container` 单独开一条"单位表达不同"的豁免。
/// `silex.toml` 里额外配置的断点由宏层按其配置宽度追加。
pub const CONTAINER_TIERS: &[(&str, &str)] = &[
    ("sm", "40rem"),
    ("md", "48rem"),
    ("lg", "64rem"),
    ("xl", "80rem"),
    ("2xl", "96rem"),
];

pub static AT_RULE_UTILITIES: &[AtRuleUtility] = &[
    // Tailwind 的 `container` 是"宽度撑满、到断点为止"的容器工具类。
    // 它与 `@container`（容器查询上下文，`container-type: inline-size`）毫无关系，
    // 此前两者产出完全相同的 CSS，写 `container mx-auto px-4` 的用户什么也得不到。
    AtRuleUtility {
        class: "container",
        groups: &[CondDecls {
            media: None,
            decls: &[("width", "100%")],
        }],
        per_breakpoint: Some("max-width"),
    },
    // `outline-hidden` 是"视觉上去掉描边，但在强制配色模式下留一条透明描边"——
    // 后半句正是它与 `outline-none` 的唯一区别，也是无障碍上的要点：
    // forced-colors 下浏览器会把 transparent 换成实色，键盘焦点因此不会消失。
    AtRuleUtility {
        class: "outline-hidden",
        groups: &[
            CondDecls {
                media: None,
                decls: &[("outline-style", "none")],
            },
            CondDecls {
                media: Some("(forced-colors: active)"),
                decls: &[
                    ("outline", "2px solid transparent"),
                    ("outline-offset", "2px"),
                ],
            },
        ],
        per_breakpoint: None,
    },
];

/// 该类名是否由 at-rule 路径承载
pub fn lookup_at_rule_utility(class: &str) -> Option<&'static AtRuleUtility> {
    AT_RULE_UTILITIES.iter().find(|u| u.class == class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_and_outline_hidden_are_registered() {
        assert!(lookup_at_rule_utility("container").is_some());
        assert!(lookup_at_rule_utility("outline-hidden").is_some());
        assert!(lookup_at_rule_utility("@container").is_none());
        assert!(lookup_at_rule_utility("outline-none").is_none());
    }

    struct NoPalette;
    impl crate::context::TwContext for NoPalette {
        fn palette_shade(&self, _: &str, _: &str) -> Option<&str> {
            None
        }
        fn palette_ramp(&self, _: &str) -> Option<[&str; 11]> {
            None
        }
    }

    /// 这张表里的类名**必须**从 `resolve_class` 里缺席，否则静态表会先命中，
    /// at-rule 分组永远不会被产出——那就是报告 §3.1 说的死代码。
    #[test]
    fn at_rule_utilities_are_absent_from_the_plain_resolver() {
        let ctx = NoPalette;
        for u in AT_RULE_UTILITIES {
            assert!(
                crate::resolver::resolve_class(u.class, &ctx).is_none(),
                "'{}' 同时存在于 AT_RULE_UTILITIES 与 resolve_class 中",
                u.class
            );
        }
    }
}
