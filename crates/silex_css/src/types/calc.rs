//! 数学表达式、量纲标记，以及运算符重载。

// 各单位的具体类型名由下面三个 `for_all_*_units!` 宏展开引用
use crate::types::units::{
    Ch, Cm, Deg, Dvh, Dvw, Em, Ex, In, Lvh, Lvw, Mm, Ms, Pc, Percent, Pt, Px, Qmm, Rad, Rem, Sec,
    Svh, Svw, Turn, Vh, Vmax, Vmin, Vw, for_all_angle_units, for_all_length_units,
    for_all_time_units,
};
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
pub struct TimeMark;

// `NumberMark` / `ColorMark` / `CssNumber` / `CssPercentage` 曾在这里声明，
// 但全仓零使用、零实现——它们只是让「量纲标记」这套说法看起来更完整。
// 真正在用的是长度、角度、时间三种量纲。

/// CSS 的 `<length>`：带长度单位的值，**不含**百分比。
///
/// 区分它和 `<length-percentage>` 是有实际用处的：`translateZ()` 与
/// `perspective()` 只接受 `<length>`，给百分比是无效的。
pub trait CssLength: Display {}
/// CSS 的 `<length-percentage>`：长度、百分比，以及两者的算式。
///
/// 算术运算符收的是这一个——`calc(100% - 10px)` 本来就是合法的。
pub trait CssLengthPercentage: Display {}
pub trait CssAngle: Display {}
pub trait CssTime: Display {}
pub trait CssColor: Display {}

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
for_all_length_units!(impl_calc_operand_via_display);
for_all_angle_units!(impl_calc_operand_via_display);
for_all_time_units!(impl_calc_operand_via_display);
impl_calc_operand_via_display!(Percent, f64, f32, i32);

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

// `CalcValue<Mark>` 的量纲标记与算术运算符和各单位一起，在 `types.rs` 的
// `impl_dimension_family!` 里统一展开。

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

/// `min(a, b, …)`，参数来自一个**同型**集合。
///
/// 迭代器只有一个 `Item` 类型，所以 `min([px(10), pct(50)])` 编译不过。混用不同
/// 单位请用 [`css_min!`](crate::css_min)；这个函数留给 `min(vec_of_px)` 这种参数
/// 本来就来自运行时集合、元素本来就同型的场合。
pub fn min<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    math_fn("min", args)
}

/// `max(a, b, …)`，参数来自一个**同型**集合。
///
/// 与 [`min`] 同样的取舍——混用不同单位请用 [`css_max!`](crate::css_max)。
pub fn max<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    math_fn("max", args)
}

/// `css_min!(px(10), pct(50))` → `min(10px, 50%)`。
///
/// 与函数版 [`min`](crate::types::min) 的分工：参数在编译期就写死、且类型可能
/// 不同时用宏；参数来自运行时集合时用函数。
///
/// 每个参数各自过一次 [`IntoCalc`](crate::types::IntoCalc) 落到
/// `CalcValue<Mark>`，数组元素因此同型；`Mark` 由参数自己的量纲反推——跨量纲
/// 混用（`css_min!(px(1), sec(1))`）仍然编译失败，这正是量纲标记要挡住的。
#[macro_export]
macro_rules! css_min {
    ($($arg:expr),+ $(,)?) => {
        $crate::types::min([$( $crate::types::IntoCalc::into_calc($arg) ),+])
    };
}

/// `css_max!(rem(1), px(16), pct(5))` → `max(1rem, 16px, 5%)`。
///
/// 见 [`css_min!`](crate::css_min)。
#[macro_export]
macro_rules! css_max {
    ($($arg:expr),+ $(,)?) => {
        $crate::types::max([$( $crate::types::IntoCalc::into_calc($arg) ),+])
    };
}

/// `clamp(<min>, <val>, <max>)`。
///
/// 三个参数各有各的类型参数——只要都属于同一个量纲就行。此前是
/// `clamp<Mark, T>(min_v: T, val: T, max_v: T)`，要求三参**同型**，于是
/// 开发文档里那句 `clamp(px(100), pct(50), px(500))` 根本编译不过
/// （`E0308 expected Px, found Percent`）。而这恰恰是 `clamp` 最典型的用法。
pub fn clamp<Mark, Lo, Val, Hi>(min_v: Lo, val: Val, max_v: Hi) -> CalcValue<Mark>
where
    Lo: Into<CalcValue<Mark>>,
    Val: Into<CalcValue<Mark>>,
    Hi: Into<CalcValue<Mark>>,
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

/// `css_clamp!(px(100), pct(50), px(500))` → `clamp(100px, 50%, 500px)`。
///
/// 纯粹为了让三个数学函数写法一致——函数版 [`clamp`](crate::types::clamp) 早就
/// 是三个独立类型参数，本来就能混用单位，两者完全等价。
#[macro_export]
macro_rules! css_clamp {
    ($lo:expr, $val:expr, $hi:expr $(,)?) => {
        $crate::types::clamp(
            $crate::types::IntoCalc::into_calc($lo),
            $crate::types::IntoCalc::into_calc($val),
            $crate::types::IntoCalc::into_calc($hi),
        )
    };
}

/// 把一个成品值放进某个量纲的数学上下文。
///
/// 此前长度那一侧是一条 blanket impl：`impl<T: Display> IntoCalc<LengthMark> for T`。
/// 任何能打印的东西——颜色、角度、裸字符串——都能变成长度，量纲标记等于没有。
/// 现在两侧都只对本量纲的具体类型开放。
pub trait IntoCalc<Mark> {
    fn into_calc(self) -> CalcValue<Mark>;
}

macro_rules! impl_into_calc {
    ($mark:ident: $($t:ty),* $(,)?) => {
        $(impl IntoCalc<$mark> for $t {
            fn into_calc(self) -> CalcValue<$mark> {
                CalcValue::new(self.to_string())
            }
        })*
    };
}

macro_rules! impl_into_calc_length {
    ($($t:ty),* $(,)?) => { impl_into_calc!(LengthMark: $($t),*); };
}
macro_rules! impl_into_calc_angle {
    ($($t:ty),* $(,)?) => { impl_into_calc!(AngleMark: $($t),*); };
}
macro_rules! impl_into_calc_time {
    ($($t:ty),* $(,)?) => { impl_into_calc!(TimeMark: $($t),*); };
}

for_all_length_units!(impl_into_calc_length);
impl_into_calc!(LengthMark: Percent);
for_all_angle_units!(impl_into_calc_angle);
for_all_time_units!(impl_into_calc_time);

macro_rules! impl_calc_identity {
    ($($mark:ident),* $(,)?) => {
        $(impl IntoCalc<$mark> for CalcValue<$mark> {
            fn into_calc(self) -> CalcValue<$mark> {
                self
            }
        })*
    };
}
impl_calc_identity!(LengthMark, AngleMark, TimeMark);

/// `min()` / `max()` / `clamp()` 收的是 `impl Into<CalcValue<Mark>>`，所以每个
/// 单位都要能转进自己的量纲——但**只能**转进自己的量纲：这正是量纲标记要挡住
/// 的东西。
macro_rules! impl_from_unit_for_calc {
    ($mark:ident: $($t:ty),* $(,)?) => {
        $(impl From<$t> for CalcValue<$mark> {
            fn from(v: $t) -> Self {
                v.into_calc()
            }
        })*
    };
}
macro_rules! impl_from_length_for_calc {
    ($($t:ty),* $(,)?) => { impl_from_unit_for_calc!(LengthMark: $($t),*); };
}
macro_rules! impl_from_angle_for_calc {
    ($($t:ty),* $(,)?) => { impl_from_unit_for_calc!(AngleMark: $($t),*); };
}
macro_rules! impl_from_time_for_calc {
    ($($t:ty),* $(,)?) => { impl_from_unit_for_calc!(TimeMark: $($t),*); };
}

for_all_length_units!(impl_from_length_for_calc);
impl_from_unit_for_calc!(LengthMark: Percent);
for_all_angle_units!(impl_from_angle_for_calc);
for_all_time_units!(impl_from_time_for_calc);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::units::{deg, px};
    use silex_core::{Runtime, SilexContext};

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

    /// 时间也是一个完整的量纲，不只是「能打印的东西」
    #[test]
    fn time_arithmetic_stays_in_its_own_dimension() {
        use crate::types::units::{ms, sec};
        assert_eq!((sec(1) - ms(200)).to_string(), "calc(1s - 200ms)");
        let c: CalcValue<TimeMark> = clamp(ms(100), sec(1), sec(2));
        assert_eq!(c.to_string(), "clamp(100ms, 1s, 2s)");
    }

    /// 长度与百分比可以混算——`calc(100% - 10px)` 本来就是合法 CSS。
    /// 跨量纲的组合由 `trybuild` 反例负责证明编译失败。
    #[test]
    fn lengths_and_percentages_mix_freely() {
        use crate::types::units::pct;
        assert_eq!((pct(100) - px(10)).to_string(), "calc(100% - 10px)");
    }

    /// `min([px(10), pct(50)])` 编译不过——迭代器只有一个 `Item` 类型。
    /// 宏版把每个参数各自落到 `CalcValue<Mark>`，数组元素这才同型
    #[test]
    fn the_macro_form_of_min_takes_mixed_units() {
        use crate::types::units::pct;
        let m: CalcValue<LengthMark> = css_min!(px(10), pct(50));
        assert_eq!(m.to_string(), "min(10px, 50%)");
    }

    #[test]
    fn the_macro_form_of_max_takes_any_number_of_arguments() {
        use crate::types::units::{pct, rem};
        let one: CalcValue<LengthMark> = css_max!(px(16));
        assert_eq!(one.to_string(), "max(16px)");
        let three: CalcValue<LengthMark> = css_max!(rem(1), px(16), pct(5));
        assert_eq!(three.to_string(), "max(1rem, 16px, 5%)");
        // 尾逗号
        let trailing: CalcValue<LengthMark> = css_max!(px(1), px(2),);
        assert_eq!(trailing.to_string(), "max(1px, 2px)");
    }

    /// 宏版与函数版对同型参数必须给出同一个结果
    #[test]
    fn the_macro_and_the_function_agree_on_homogeneous_arguments() {
        let by_macro: CalcValue<LengthMark> = css_min!(px(10), px(20));
        let by_fn: CalcValue<LengthMark> = min([px(10), px(20)]);
        assert_eq!(by_macro, by_fn);
    }

    /// 参数本身是算式时要补括号，不能套出 `min(calc(…))`
    #[test]
    fn the_macro_keeps_nested_expressions_as_operands() {
        use crate::types::units::pct;
        let m: CalcValue<LengthMark> = css_min!(px(10) + px(5), pct(50));
        assert_eq!(m.to_string(), "min((10px + 5px), 50%)");
        assert!(!m.to_string().contains("calc("));
    }

    /// 时间与角度也走同一条路——`Mark` 由参数自己反推，不需要标注
    #[test]
    fn the_macro_infers_the_dimension_from_its_arguments() {
        use crate::types::units::{deg, ms, sec, turn};
        assert_eq!(css_min!(ms(100), sec(1)).to_string(), "min(100ms, 1s)");
        assert_eq!(css_max!(deg(90), turn(1)).to_string(), "max(90deg, 1turn)");
    }

    /// 在属性调用点上没有类型标注可写，`Mark` 必须完全由参数反推出来
    #[test]
    fn the_macro_needs_no_annotation_at_a_property_call_site() {
        use crate::builder::Style;
        use crate::types::units::{pct, rem, vw};
        let mut runtime = Runtime::new();
        runtime
            .child(|scope| {
                let error_handler = scope
                    .error_handler(|_| {})
                    .expect("test error handler should register");
                let css = Style::new(SilexContext::new(scope, error_handler))
                    .width(css_min!(px(600), pct(100)))
                    .expect("width should build")
                    .font_size(css_max!(rem(1), vw(4)))
                    .expect("font-size should build")
                    .render()
                    .css;
                assert!(css.contains("width: min(600px, 100%)"), "{css}");
                assert!(css.contains("font-size: max(1rem, 4vw)"), "{css}");
            })
            .expect("test context should initialize");
    }

    #[test]
    fn the_macro_form_of_clamp_matches_the_function() {
        use crate::types::units::pct;
        let by_macro: CalcValue<LengthMark> = css_clamp!(px(100), pct(50), px(500));
        assert_eq!(by_macro.to_string(), "clamp(100px, 50%, 500px)");
        assert_eq!(by_macro, clamp(px(100), pct(50), px(500)));
    }

    /// 开发文档里的 `clamp(px(100), pct(50), px(500))` 曾编译不过：
    /// `clamp<Mark, T>` 要求三参同型（`E0308 expected Px, found Percent`），
    /// 而这恰恰是 `clamp` 最典型的用法
    #[test]
    fn clamp_takes_three_different_types_from_one_dimension() {
        use crate::types::units::pct;
        let c: CalcValue<LengthMark> = clamp(px(100), pct(50), px(500));
        assert_eq!(c.to_string(), "clamp(100px, 50%, 500px)");
    }
}
