//! 颜色前缀的**唯一真值表**。
//!
//! 报告 §3.1 / 附录 B 点名了三份互相漂移的颜色前缀映射：
//! `silex_macros` 的 `color_prefix_to_prop`、同 crate 的 `palette::ORDERED_PREFIXES`、
//! 以及 codegen 的 `colors.rs`。三份覆盖范围各不相同，且因为静态表优先命中，
//! 冲突时永远是 codegen 那份胜出——`ring-<color>` 曾经因此被映射到 `outline-color`，
//! macro 侧"正确"的映射成了永不执行的死代码。
//!
//! 现在只有这一张表。前缀的**顺序即优先级**（最长/最具体在前），
//! 两侧都必须通过 [`match_color_prefix`] 查询，禁止再各自维护副本。

/// `divide-*` 的伴生选择器：声明落在相邻子元素之间，而不是元素自身
pub const DIVIDE_SELECTOR: &str = "& > :not([hidden]) ~ :not([hidden])";

/// ring 体系的 `box-shadow` 载体。
///
/// 四段依次是：内描边（`inset-ring-*`）、ring 偏移、ring 本体、以及用户自己的 `shadow-*`。
/// 每一段的宽度默认 `0px`，未使用的段渲染出来是空的，因此可以无条件铺这一条声明。
///
/// 内描边段是必须的：`inset-ring-<color>` 写进 `--tw-inset-ring-color` 之后
/// 得有人消费它，否则就是 §2.4 那个 ring 缺陷的翻版——颜色写进了一个没人读的变量，
/// 用户看到的是"什么都没发生"。
pub const RING_BOX_SHADOW: &str = "inset 0 0 0 var(--tw-inset-ring-width, 0px) var(--tw-inset-ring-color, currentcolor), var(--tw-ring-inset, ) 0 0 0 var(--tw-ring-offset-width, 0px) var(--tw-ring-offset-color, #0000), 0 0 0 var(--tw-ring-width, 0px) var(--tw-ring-color, rgba(59, 130, 246, 0.5)), var(--tw-shadow, 0 0 #0000)";

/// ring 体系里"值是尺寸"时落到哪个宽度变量。
///
/// 顺序即优先级（`ring-offset-` 必须先于 `ring-`）。
/// 数值路径（`ring-2`）与任意值路径（`ring-[3px]`）都读这张表——此前任意值那边是
/// `match clean_prefix { "ring" => …, "ring-offset" => … }` 的硬编码，
/// 漏了 `inset-ring`，于是 `inset-ring-[3px]` 掉进颜色前缀表，产出
/// `--tw-inset-ring-color: 3px` 这种非法 CSS。
pub const RING_WIDTH_PREFIXES: &[(&str, &str)] = &[
    ("ring-offset", "--tw-ring-offset-width"),
    ("inset-ring", "--tw-inset-ring-width"),
    ("ring", "--tw-ring-width"),
];

/// 该前缀在"值是尺寸"时对应的 ring 宽度变量
pub fn ring_width_prop(prefix: &str) -> Option<&'static str> {
    RING_WIDTH_PREFIXES
        .iter()
        .find(|(p, _)| *p == prefix)
        .map(|(_, prop)| *prop)
}

/// 颜色前缀命中后需要附带产出的固定声明。
///
/// 这些此前都是散落在两侧 resolver 里的 `if prefix == "..."` 硬编码分支（报告 §3.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorCompanion {
    /// ring 颜色写进 CSS 变量后，还得铺一条 `box-shadow` 才能真正显示出来
    RingShadow,
    /// `from-*`：Silex 把渐变方向内联进了 `linear-gradient(to right, var(--tw-gradient-stops))`，
    /// 因此 `--tw-gradient-stops` 必须由色标工具类自己拼出来，否则整条声明无效
    GradientFrom,
    /// `via-*`：把 stops 从两段扩成三段
    GradientVia,
}

impl ColorCompanion {
    /// 伴生声明列表 `(属性, 固定值)`。
    ///
    /// 值是固定文本、与用户写的颜色无关，因此可以直接作为数据共享——
    /// `silex_macros` 的 `theme()` 路径与本 crate 的颜色路径都读这里，
    /// 不再各自 `push` 一遍字面量。
    pub const fn decls(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::RingShadow => &[("box-shadow", RING_BOX_SHADOW)],
            Self::GradientFrom => &[
                ("--tw-gradient-to", "rgb(255 255 255 / 0)"),
                (
                    "--tw-gradient-stops",
                    "var(--tw-gradient-from), var(--tw-gradient-to)",
                ),
            ],
            Self::GradientVia => &[(
                "--tw-gradient-stops",
                "var(--tw-gradient-from), var(--tw-gradient-via), var(--tw-gradient-to)",
            )],
        }
    }
}

/// 一个颜色前缀的完整规则描述
#[derive(Debug, Clone, Copy)]
pub struct ColorPrefixRule {
    /// 含尾部 `-`，便于直接 `strip_prefix`
    pub prefix: &'static str,
    /// 颜色值写入的目标属性
    pub props: &'static [&'static str],
    /// 伴生声明
    pub companion: Option<ColorCompanion>,
    /// 伴生选择器：`None` 表示作用在元素自身
    pub selector: Option<&'static str>,
}

const fn rule(prefix: &'static str, props: &'static [&'static str]) -> ColorPrefixRule {
    ColorPrefixRule {
        prefix,
        props,
        companion: None,
        selector: None,
    }
}

const fn rule_with(
    prefix: &'static str,
    props: &'static [&'static str],
    companion: ColorCompanion,
) -> ColorPrefixRule {
    ColorPrefixRule {
        prefix,
        props,
        companion: Some(companion),
        selector: None,
    }
}

const fn rule_scoped(
    prefix: &'static str,
    props: &'static [&'static str],
    selector: &'static str,
) -> ColorPrefixRule {
    ColorPrefixRule {
        prefix,
        props,
        companion: None,
        selector: Some(selector),
    }
}

/// 全部颜色前缀，**顺序即优先级**。
///
/// 排序规则：更长/更具体的前缀必须排在它的短前缀之前
/// （`ring-offset-` 先于 `ring-`、`border-bs-` 先于 `border-b-`、`text-shadow-` 先于 `text-`），
/// 否则短前缀会先吃掉输入。改动顺序前请先想清楚会不会遮蔽后面的条目。
pub static COLOR_PREFIX_RULES: &[ColorPrefixRule] = &[
    rule_with(
        "ring-offset-",
        &["--tw-ring-offset-color"],
        ColorCompanion::RingShadow,
    ),
    rule_with("ring-", &["--tw-ring-color"], ColorCompanion::RingShadow),
    rule("scrollbar-thumb-", &["scrollbar-color"]),
    rule("scrollbar-track-", &["scrollbar-color"]),
    rule("inset-shadow-", &["--tw-inset-shadow-color"]),
    rule("drop-shadow-", &["--tw-drop-shadow-color"]),
    rule("text-shadow-", &["--tw-text-shadow-color"]),
    rule_scoped("placeholder-", &["color"], "&::placeholder"),
    rule("decoration-", &["text-decoration-color"]),
    rule_with(
        "inset-ring-",
        &["--tw-inset-ring-color"],
        ColorCompanion::RingShadow,
    ),
    rule("border-bs-", &["border-block-start-color"]),
    rule("border-be-", &["border-block-end-color"]),
    rule("border-b-", &["border-bottom-color"]),
    rule("border-t-", &["border-top-color"]),
    rule("border-l-", &["border-left-color"]),
    rule("border-r-", &["border-right-color"]),
    rule("border-s-", &["border-inline-start-color"]),
    rule("border-e-", &["border-inline-end-color"]),
    rule("border-x-", &["border-inline-color"]),
    rule("border-y-", &["border-block-color"]),
    rule("outline-", &["outline-color"]),
    rule("border-", &["border-color"]),
    rule("accent-", &["accent-color"]),
    rule_scoped("divide-", &["border-color"], DIVIDE_SELECTOR),
    rule("stroke-", &["stroke"]),
    rule("shadow-", &["--tw-shadow-color"]),
    rule("caret-", &["caret-color"]),
    rule("text-", &["color"]),
    rule("fill-", &["fill"]),
    rule_with(
        "from-",
        &["--tw-gradient-from"],
        ColorCompanion::GradientFrom,
    ),
    rule_with("via-", &["--tw-gradient-via"], ColorCompanion::GradientVia),
    rule("bg-", &["background-color"]),
    rule("to-", &["--tw-gradient-to"]),
];

/// 按优先级切分出颜色前缀与其后的颜色词条。
///
/// 返回 `(规则, 颜色部分)`，例如 `"border-t-red-500"` → `(border-top-color 规则, "red-500")`。
/// 注意这里只做**前缀切分**，颜色词条本身是否合法由调用方用
/// [`crate::color::parse_color_value`] 判定——同一个前缀可能既接颜色又接尺寸
/// （`ring-2` / `ring-blue-500`），切分成功不等于命中。
pub fn match_color_prefix(class: &str) -> Option<(&'static ColorPrefixRule, &str)> {
    COLOR_PREFIX_RULES
        .iter()
        .find_map(|r| class.strip_prefix(r.prefix).map(|rest| (r, rest)))
}

/// 按前缀名（不含尾部 `-`）精确查表，供任意值路径 `bg-[...]` 等使用
pub fn lookup_color_prefix(prefix: &str) -> Option<&'static ColorPrefixRule> {
    COLOR_PREFIX_RULES
        .iter()
        .find(|r| r.prefix.len() == prefix.len() + 1 && r.prefix.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 表的顺序就是语义。短前缀排到长前缀前面会静默遮蔽后者，
    /// 而被遮蔽的那条永远不会命中——正是 §3.1 里 ring 死代码的成因。
    #[test]
    fn longer_prefixes_are_never_shadowed_by_shorter_ones() {
        for (i, long) in COLOR_PREFIX_RULES.iter().enumerate() {
            for short in &COLOR_PREFIX_RULES[..i] {
                assert!(
                    !long.prefix.starts_with(short.prefix),
                    "'{}' 排在 '{}' 之后，会被后者提前吃掉，永远无法命中",
                    long.prefix,
                    short.prefix
                );
            }
        }
    }

    #[test]
    fn matches_most_specific_prefix() {
        let (r, rest) = match_color_prefix("border-t-red-500").unwrap();
        assert_eq!(r.props, &["border-top-color"]);
        assert_eq!(rest, "red-500");

        let (r, rest) = match_color_prefix("text-shadow-red-500").unwrap();
        assert_eq!(r.props, &["--tw-text-shadow-color"]);
        assert_eq!(rest, "red-500");

        let (r, rest) = match_color_prefix("ring-offset-blue-500").unwrap();
        assert_eq!(r.props, &["--tw-ring-offset-color"]);
        assert_eq!(r.companion, Some(ColorCompanion::RingShadow));
        assert_eq!(rest, "blue-500");
    }

    #[test]
    fn lookup_by_bare_prefix_name() {
        assert_eq!(
            lookup_color_prefix("bg").unwrap().props,
            &["background-color"]
        );
        assert_eq!(
            lookup_color_prefix("ring").unwrap().props,
            &["--tw-ring-color"]
        );
        assert!(lookup_color_prefix("nope").is_none());
    }
}
