//! `divide-*` / `space-*`：声明落在**相邻子元素之间**，而不是元素自身。
//!
//! 报告 §3.2 把这一族点名为"需要注入伴生选择器、只能硬编码"的例子。此前它确实
//! 是硬编码的，而且是**两份**：`silex_macros` 的 `resolve_pattern_utility` 里有
//! 80 行 `strip_prefix("divide-")` 长链（带 [`DIVIDE_SELECTOR`]），core 这边的
//! `divide-x` / `space-x-` 又各有一份**不带选择器**的映射。因为宏那份排在前面，
//! core 这份是永远不会命中的死代码——`ring-<color>` 那个坑的翻版，
//! 区别只在于它还没被人踩到。
//!
//! 现在整族收敛成下面这张 [`BETWEEN_AXES`] 表：主属性、恒零的对侧属性、
//! reverse 变量、值的单位约定全部是数据，求值器只有一个。

use crate::{
    prefix::DIVIDE_SELECTOR,
    resolver::dynamic::resolve_length_val,
    value::{TwDecl, TwRuleSet},
};

/// 轴上数值后缀的单位约定
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisUnit {
    /// `divide-x-2` → `2px`（边框宽度按字面像素）
    Pixel,
    /// `space-x-2` → `0.5rem`（间距档位，支持负号）
    Spacing,
}

/// 一个"落在子元素之间"的轴
struct BetweenAxis {
    /// 不含尾部 `-`
    prefix: &'static str,
    /// 值写入的属性
    prop: &'static str,
    /// 恒为 `0px` 的对侧属性——不写它，前一个工具类留下的对侧值会残留
    zero_prop: &'static str,
    /// `<prefix>-reverse` 设置的变量
    reverse_var: &'static str,
    /// 无数值后缀时的默认值（`divide-x` ≡ `divide-x-1`）；`space-x` 本身不是类名
    bare_value: Option<&'static str>,
    unit: AxisUnit,
}

const BETWEEN_AXES: &[BetweenAxis] = &[
    BetweenAxis {
        prefix: "divide-x",
        prop: "border-left-width",
        zero_prop: "border-right-width",
        reverse_var: "--tw-divide-x-reverse",
        bare_value: Some("1px"),
        unit: AxisUnit::Pixel,
    },
    BetweenAxis {
        prefix: "divide-y",
        prop: "border-top-width",
        zero_prop: "border-bottom-width",
        reverse_var: "--tw-divide-y-reverse",
        bare_value: Some("1px"),
        unit: AxisUnit::Pixel,
    },
    BetweenAxis {
        prefix: "space-x",
        prop: "margin-left",
        zero_prop: "margin-right",
        reverse_var: "--tw-space-x-reverse",
        bare_value: None,
        unit: AxisUnit::Spacing,
    },
    BetweenAxis {
        prefix: "space-y",
        prop: "margin-top",
        zero_prop: "margin-bottom",
        reverse_var: "--tw-space-y-reverse",
        bare_value: None,
        unit: AxisUnit::Spacing,
    },
];

/// `divide-<线型>`：线型作用在同一个伴生选择器上
const DIVIDE_STYLES: &[&str] = &["solid", "dashed", "dotted", "double", "none"];

/// 把一组声明包进伴生选择器
fn between(decls: Vec<TwDecl>) -> Vec<TwRuleSet> {
    vec![TwRuleSet::scoped(Some(DIVIDE_SELECTOR), decls)]
}

/// 解析 `divide-*` / `space-*` 的尺寸与线型部分。
///
/// `divide-<颜色>` 不在这里——颜色统一由 [`crate::color::resolve_color_utility`]
/// 按 [`crate::prefix::COLOR_PREFIX_RULES`] 处理，那张表里 `divide-` 挂的是同一个
/// [`DIVIDE_SELECTOR`]。
pub fn resolve_between_rules(class_name: &str) -> Option<Vec<TwRuleSet>> {
    if let Some(style) = class_name.strip_prefix("divide-")
        && let Some(&style) = DIVIDE_STYLES.iter().find(|&&s| s == style)
    {
        return Some(between(vec![TwDecl::new("border-style", style)]));
    }

    // 负号只对间距档位有意义（`-space-x-2`）；`-divide-x-2` 不是合法类名
    let (is_negative, base) = match class_name.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, class_name),
    };

    for axis in BETWEEN_AXES {
        let Some(rest) = base.strip_prefix(axis.prefix) else {
            continue;
        };

        if rest == "-reverse" && !is_negative {
            return Some(between(vec![TwDecl::new(axis.reverse_var, "1")]));
        }

        let value = if rest.is_empty() {
            if is_negative {
                return None;
            }
            axis.bare_value?.to_string()
        } else {
            let val_str = rest.strip_prefix('-')?;
            match axis.unit {
                AxisUnit::Pixel => {
                    if is_negative {
                        return None;
                    }
                    format!("{}px", val_str.parse::<f64>().ok()?)
                }
                AxisUnit::Spacing => {
                    let signed = if is_negative {
                        format!("-{}", val_str)
                    } else {
                        val_str.to_string()
                    };
                    resolve_length_val(&signed)?
                }
            }
        };

        return Some(between(vec![
            TwDecl::new(axis.prop, value),
            TwDecl::new(axis.zero_prop, "0px"),
        ]));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decls(class: &str) -> Vec<(&'static str, String)> {
        let sets = resolve_between_rules(class).expect(class);
        assert_eq!(sets.len(), 1);
        assert_eq!(
            sets[0].selector,
            Some(DIVIDE_SELECTOR),
            "{class} 的声明必须落在子元素之间，而不是元素自身"
        );
        sets[0]
            .decls
            .iter()
            .map(|d| (d.prop, d.value.to_string()))
            .collect()
    }

    #[test]
    fn divide_widths_carry_the_companion_selector() {
        assert_eq!(
            decls("divide-x"),
            [
                ("border-left-width", "1px".into()),
                ("border-right-width", "0px".into())
            ]
        );
        assert_eq!(
            decls("divide-y-4"),
            [
                ("border-top-width", "4px".into()),
                ("border-bottom-width", "0px".into())
            ]
        );
    }

    #[test]
    fn divide_styles_and_reverse() {
        assert_eq!(decls("divide-dashed"), [("border-style", "dashed".into())]);
        assert_eq!(
            decls("divide-x-reverse"),
            [("--tw-divide-x-reverse", "1".into())]
        );
    }

    #[test]
    fn space_uses_the_spacing_scale_and_accepts_a_sign() {
        assert_eq!(
            decls("space-x-4"),
            [
                ("margin-left", "1rem".into()),
                ("margin-right", "0px".into())
            ]
        );
        assert_eq!(
            decls("-space-y-2"),
            [
                ("margin-top", "-0.5rem".into()),
                ("margin-bottom", "0px".into())
            ]
        );
        assert_eq!(
            decls("space-y-reverse"),
            [("--tw-space-y-reverse", "1".into())]
        );
    }

    /// 颜色词条不能被这里吃掉——它归颜色前缀表管
    #[test]
    fn color_suffixes_are_left_alone() {
        assert!(resolve_between_rules("divide-slate-200").is_none());
        assert!(resolve_between_rules("space-x-red-500").is_none());
        assert!(resolve_between_rules("-divide-x-2").is_none());
    }
}
