//! 宿主无关的规则模型。
//!
//! 刻意做得很薄：一条声明就是 `(属性名, CSS 值文本)`，外加一个**共享的分类器**
//! [`classify`] 把值文本判成关键字 / 数值 / Hex / 字面量。
//!
//! 之所以不设计成带类型的值枚举，是因为两侧真正需要保持一致的东西只有两样：
//! 值渲染成什么文本、以及它被归成哪一类。把这两件事各留一份实现，
//! 就还是报告 §3.1 那种"没有任何机制保证一致"的局面。共用同一个函数才是保证。
//!
//! `silex_macros` 侧 `UtilityValue` 的 `ThemeVar` / `DynamicExpr(syn::Expr)` 两个变体
//! 对应 `bg-theme(primary)` 与 `p-[$(sig)]` 这两处 Silex 语法扩展，codegen 没有对应输入，
//! 因此不下沉到 core，也就不必把 `syn` 拖进来。

use std::borrow::Cow;

/// 一条声明
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwDecl {
    pub prop: &'static str,
    pub value: Cow<'static, str>,
}

impl TwDecl {
    #[inline]
    pub fn new(prop: &'static str, value: impl Into<Cow<'static, str>>) -> Self {
        Self {
            prop,
            value: value.into(),
        }
    }
}

/// 一组共享同一选择器上下文的声明。
///
/// `selector` 是**伴生选择器**：`None` 表示声明落在元素自身，
/// `Some(sel)` 表示其实落在别处——`divide-*` 落在
/// `& > :not([hidden]) ~ :not([hidden])`、`placeholder-*` 落在 `&::placeholder`。
/// 报告 §3.2 指出这类信息此前散落成硬编码分支，现在收进数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwRuleSet {
    pub selector: Option<&'static str>,
    pub decls: Vec<TwDecl>,
}

impl TwRuleSet {
    #[inline]
    pub fn plain(decls: Vec<TwDecl>) -> Self {
        Self {
            selector: None,
            decls,
        }
    }

    #[inline]
    pub fn scoped(selector: Option<&'static str>, decls: Vec<TwDecl>) -> Self {
        Self { selector, decls }
    }
}

/// 值的类别。
///
/// codegen 用它决定静态表里写哪个 `StaticVal` 变体，
/// macro 用它决定构造哪个 `UtilityValue` 变体——同一个判定函数，两侧不可能分叉。
#[derive(Debug, Clone, PartialEq)]
pub enum TwValueKind {
    /// 关键字或可安全内联的函数值：`flex`、`repeat(4, minmax(0, 1fr))`
    Keyword,
    /// 数值 + 单位：`1rem` → `(1.0, "rem")`，`0.5` → `(0.5, "")`
    Numeric(f64, &'static str),
    /// 颜色 Hex 字面量：`#1e293b`
    Hex,
    /// 其余复合字面量：`rgba(0, 0, 0, .5)`、`span 2 / span 2`
    Literal,
    /// ring 体系的 `box-shadow` 载体——值很长且固定，两侧都按常量引用而不是内联文本
    RingShadow,
}

/// 识别出的数值单位。顺序有意义：`rem` 必须先于 `em`，否则 `1rem` 会被判成 `1r` + `em`。
const NUMERIC_UNITS: &[&str] = &["rem", "px", "%", "vw", "vh", "em", "deg", "ms", "s"];

/// 尝试把值文本解析成 `(数值, 单位)`
pub fn try_parse_numeric(val: &str) -> Option<(f64, &'static str)> {
    if let Ok(v) = val.parse::<f64>() {
        return Some((v, ""));
    }
    NUMERIC_UNITS.iter().find_map(|&unit| {
        let head = val.strip_suffix(unit)?;
        Some((head.parse::<f64>().ok()?, unit))
    })
}

/// 可以安全按"关键字"对待的函数值前缀。
///
/// 与 [`TwValueKind::Literal`] 的区别纯粹是承载方式（静态字符串 vs 拥有所有权的串），
/// 语义完全相同；这里保留分类是为了让生成的静态表继续复用 `&'static str`。
const KEYWORD_FUNCTIONS: &[&str] = &[
    "linear-gradient(",
    "radial-gradient(",
    "conic-gradient(",
    "calc(",
    "rotate(",
    "translateX(",
    "translateY(",
    "blur(",
    "minmax(",
    "repeat(",
];

/// 判定一个 CSS 值文本的类别。**两侧唯一的分类入口。**
pub fn classify(val: &str) -> TwValueKind {
    if val.contains("var(--tw-ring-inset") || val.contains("0 0 0 var(--tw-ring-offset-width") {
        return TwValueKind::RingShadow;
    }
    if val.starts_with('#') {
        return TwValueKind::Hex;
    }
    if let Some((v, unit)) = try_parse_numeric(val) {
        return TwValueKind::Numeric(v, unit);
    }
    if val.contains('(') || val.contains(' ') || val.contains('/') || val.contains(',') {
        if KEYWORD_FUNCTIONS.iter().any(|f| val.starts_with(f)) {
            return TwValueKind::Keyword;
        }
        return TwValueKind::Literal;
    }
    TwValueKind::Keyword
}

/// 去掉浮点尾随零的紧凑数值格式化（`1.0` → `1`、`0.375` → `0.375`）
pub fn format_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{:.0}", v)
    } else {
        format!("{:.6}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

/// 渲染 `(数值, 单位)` 为 CSS 文本
pub fn format_numeric(v: f64, unit: &str) -> String {
    format!("{}{}", format_num(v), unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_rendering_drops_trailing_zeros() {
        assert_eq!(format_numeric(1.0, "rem"), "1rem");
        assert_eq!(format_numeric(0.375, "rem"), "0.375rem");
        assert_eq!(format_numeric(0.5, ""), "0.5");
        assert_eq!(format_numeric(-90.0, "deg"), "-90deg");
    }

    /// `rem` 必须先于 `em` 被尝试，否则 `1rem` 会被切成 `1r` + `em` 而解析失败
    #[test]
    fn rem_is_not_mistaken_for_em() {
        assert_eq!(try_parse_numeric("1rem"), Some((1.0, "rem")));
        assert_eq!(try_parse_numeric("2em"), Some((2.0, "em")));
        assert_eq!(try_parse_numeric("300ms"), Some((300.0, "ms")));
        assert_eq!(try_parse_numeric("flex"), None);
    }

    #[test]
    fn classifies_each_value_shape() {
        assert_eq!(classify("flex"), TwValueKind::Keyword);
        assert_eq!(classify("#1e293b"), TwValueKind::Hex);
        assert_eq!(classify("1rem"), TwValueKind::Numeric(1.0, "rem"));
        assert_eq!(classify("rgba(0, 0, 0, 0.5)"), TwValueKind::Literal);
        assert_eq!(classify("repeat(4, minmax(0, 1fr))"), TwValueKind::Keyword);
        assert_eq!(
            classify(crate::prefix::RING_BOX_SHADOW),
            TwValueKind::RingShadow
        );
    }
}
