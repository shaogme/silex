use crate::types::units::{Deg, Em, Percent, Px, Rad, Rem, Turn, Vh, Vw};
use std::fmt::{Display, Formatter, Result};
use std::marker::PhantomData;

// ==========================================
// 核心标记 Trait 与 量纲 (Marks & Marker Traits)
// ==========================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LengthMark;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AngleMark;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberMark;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMark;

pub trait CssLength: Display {}
pub trait CssAngle: Display {}
pub trait CssColor: Display {}
pub trait CssNumber: Display {}
pub trait CssPercentage: Display {}

/// 一个 CSS 数学表达式或数值。
///
/// `wrapped` 记录 `expr` 是否已经是**成品值**：
/// - `true`：`10px`、`calc(a + b)`、`min(a, b)` 这样可以直接写进声明的东西；
/// - `false`：`10px + 5px` 这种裸算式，渲染时必须补上 `calc()` 外壳。
///
/// 此前算术结果不带外壳就直接进 CSS（`width: (10px  +  5px)`），既无效、
/// 还因为运算符自带空格而产生双空格。
#[derive(Clone, Debug, PartialEq)]
pub struct CalcValue<Mark> {
    expr: String,
    wrapped: bool,
    _mark: PhantomData<Mark>,
}

impl<Mark> Default for CalcValue<Mark> {
    fn default() -> Self {
        Self {
            expr: String::new(),
            wrapped: true,
            _mark: PhantomData,
        }
    }
}

/// 能作为数学表达式操作数的类型。
///
/// 需要它是因为嵌套算式不能直接用 `Display`：`CalcValue` 的 `Display` 会补
/// `calc()` 外壳，套进另一个算式里就成了 `calc(calc(a + b) + c)`。
pub trait CalcOperand {
    /// 作为操作数时的文本：成品值原样返回，裸算式补括号保住优先级。
    fn calc_operand(&self) -> String;
}

macro_rules! impl_calc_operand_via_display {
    ($($t:ty),* $(,)?) => {
        $(impl CalcOperand for $t {
            #[inline]
            fn calc_operand(&self) -> String { self.to_string() }
        })*
    };
}
impl_calc_operand_via_display!(Px, Percent, Rem, Em, Vw, Vh, Deg, Rad, Turn, f64, f32, i32);

impl<Mark> CalcOperand for CalcValue<Mark> {
    fn calc_operand(&self) -> String {
        if self.wrapped {
            self.expr.clone()
        } else {
            format!("({})", self.expr)
        }
    }
}

impl<Mark> CalcValue<Mark> {
    /// 用一个**成品值**构造（`10px`、`var(--x)`、`calc(…)`）。
    pub fn new(s: String) -> Self {
        Self {
            expr: s,
            wrapped: true,
            _mark: PhantomData,
        }
    }

    /// 用一个**裸算式**构造，渲染时会补 `calc()`。
    pub fn expression(s: String) -> Self {
        Self {
            expr: s,
            wrapped: false,
            _mark: PhantomData,
        }
    }

    pub fn binary<L: CalcOperand, R: CalcOperand>(l: L, op: &'static str, r: R) -> Self {
        Self::expression(format!("{} {} {}", l.calc_operand(), op, r.calc_operand()))
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.expr.is_empty()
    }

    /// 表达式文本（不含 `calc()` 外壳）。渲染结果请用 `Display`。
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.expr
    }
    #[inline]
    pub fn into_inner(self) -> String {
        self.expr
    }
    #[inline]
    pub fn set_value(&mut self, s: impl Into<String>) {
        self.expr = s.into();
        self.wrapped = true;
    }
    #[inline]
    pub fn with_value(mut self, s: impl Into<String>) -> Self {
        self.set_value(s);
        self
    }
    #[inline]
    pub fn set_expression(&mut self, s: impl Into<String>) {
        self.expr = s.into();
        self.wrapped = false;
    }
    #[inline]
    pub fn with_expression(mut self, s: impl Into<String>) -> Self {
        self.set_expression(s);
        self
    }
    #[inline]
    pub fn update(&mut self, f: impl FnOnce(&mut String)) {
        f(&mut self.expr);
    }
}

impl<Mark> Display for CalcValue<Mark> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.expr.is_empty() {
            Ok(())
        } else if self.wrapped {
            write!(f, "{}", self.expr)
        } else {
            write!(f, "calc({})", self.expr)
        }
    }
}

impl CssLength for CalcValue<LengthMark> {}
impl CssAngle for CalcValue<AngleMark> {}

impl<Mark> From<CalcValue<Mark>> for String {
    fn from(v: CalcValue<Mark>) -> Self {
        v.to_string()
    }
}

/// 显式补一层 `calc()`。
///
/// 算术运算的结果本身已经会渲染成 `calc(…)`，所以这个函数主要用于把一个
/// 成品值也放进数学上下文；对已经是 `calc(…)` 的输入不会再套一层。
pub fn calc<Mark>(v: CalcValue<Mark>) -> CalcValue<Mark> {
    if v.wrapped {
        CalcValue::new(format!("calc({})", v.expr))
    } else {
        CalcValue::new(v.to_string())
    }
}

fn math_fn<Mark, T, I>(name: &str, args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    let mut s = format!("{}(", name);
    let mut empty = true;
    for arg in args {
        let v: CalcValue<Mark> = arg.into();
        if !v.is_empty() {
            if !empty {
                s.push_str(", ");
            }
            s.push_str(&v.calc_operand());
            empty = false;
        }
    }
    if empty {
        // `min()` / `max()` 不接受空参数表。调试构建下直接把问题暴露在调用处；
        // 发布构建下退化为空值——声明整体作废，好过写出 `min()` 这种无效函数
        debug_assert!(false, "{name}() 至少需要一个参数");
        CalcValue::default()
    } else {
        s.push(')');
        CalcValue::new(s)
    }
}

pub fn min<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    math_fn("min", args)
}

pub fn max<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    math_fn("max", args)
}

pub fn clamp<Mark, T>(min_v: T, val: T, max_v: T) -> CalcValue<Mark>
where
    T: Into<CalcValue<Mark>>,
{
    let min_v: CalcValue<Mark> = min_v.into();
    let val: CalcValue<Mark> = val.into();
    let max_v: CalcValue<Mark> = max_v.into();
    CalcValue::new(format!(
        "clamp({}, {}, {})",
        min_v.calc_operand(),
        val.calc_operand(),
        max_v.calc_operand()
    ))
}

pub trait IntoCalc<Mark> {
    fn into_calc(self) -> CalcValue<Mark>;
}

impl<T: Display> IntoCalc<LengthMark> for T {
    fn into_calc(self) -> CalcValue<LengthMark> {
        CalcValue::new(self.to_string())
    }
}

impl IntoCalc<AngleMark> for Deg {
    fn into_calc(self) -> CalcValue<AngleMark> {
        CalcValue::new(self.to_string())
    }
}
impl IntoCalc<AngleMark> for Rad {
    fn into_calc(self) -> CalcValue<AngleMark> {
        CalcValue::new(self.to_string())
    }
}
impl IntoCalc<AngleMark> for Turn {
    fn into_calc(self) -> CalcValue<AngleMark> {
        CalcValue::new(self.to_string())
    }
}
impl IntoCalc<AngleMark> for CalcValue<AngleMark> {
    fn into_calc(self) -> CalcValue<AngleMark> {
        self
    }
}

impl From<Deg> for CalcValue<AngleMark> {
    fn from(v: Deg) -> Self {
        v.into_calc()
    }
}
impl From<Rad> for CalcValue<AngleMark> {
    fn from(v: Rad) -> Self {
        v.into_calc()
    }
}
impl From<Turn> for CalcValue<AngleMark> {
    fn from(v: Turn) -> Self {
        v.into_calc()
    }
}

impl From<Px> for CalcValue<LengthMark> {
    fn from(v: Px) -> Self {
        v.into_calc()
    }
}
impl From<Percent> for CalcValue<LengthMark> {
    fn from(v: Percent) -> Self {
        v.into_calc()
    }
}
impl From<Rem> for CalcValue<LengthMark> {
    fn from(v: Rem) -> Self {
        v.into_calc()
    }
}
impl From<Em> for CalcValue<LengthMark> {
    fn from(v: Em) -> Self {
        v.into_calc()
    }
}
impl From<Vw> for CalcValue<LengthMark> {
    fn from(v: Vw) -> Self {
        v.into_calc()
    }
}
impl From<Vh> for CalcValue<LengthMark> {
    fn from(v: Vh) -> Self {
        v.into_calc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::units::{deg, px};

    /// 算术结果必须自带 `calc()` 外壳：`width: (10px + 5px)` 不是合法 CSS
    #[test]
    fn arithmetic_wraps_itself_in_calc() {
        assert_eq!((px(10) + px(5)).to_string(), "calc(10px + 5px)");
        assert_eq!((px(10) * 2.0).to_string(), "calc(10px * 2)");
        assert_eq!((deg(90) - deg(30)).to_string(), "calc(90deg - 30deg)");
    }

    /// 运算符自带空格曾导致 `(10px  +  5px)` 的双空格
    #[test]
    fn operators_do_not_double_their_spaces() {
        assert!(!(px(10) + px(5)).to_string().contains("  "));
    }

    /// 嵌套算式补括号保住优先级，且不会套出 `calc(calc(…))`
    #[test]
    fn nested_expressions_use_parentheses_not_nested_calc() {
        let v = (px(10) + px(5)) * 2.0;
        assert_eq!(v.to_string(), "calc((10px + 5px) * 2)");
        assert!(!v.to_string().contains("calc(calc"));
    }

    /// 显式 `calc()` 不再产生冗余括号
    #[test]
    fn explicit_calc_has_no_redundant_parentheses() {
        assert_eq!(calc(px(10) + px(5)).to_string(), "calc(10px + 5px)");
    }

    #[test]
    fn math_functions_render_as_complete_values() {
        let m: CalcValue<LengthMark> = min([px(10), px(20)]);
        assert_eq!(m.to_string(), "min(10px, 20px)");
        let c: CalcValue<LengthMark> = clamp(px(1), px(2), px(3));
        assert_eq!(c.to_string(), "clamp(1px, 2px, 3px)");
    }
}
