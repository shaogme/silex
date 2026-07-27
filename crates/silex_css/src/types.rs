use crate::for_all_properties;
use silex_core::{
    Rx, RxValueKind,
    reactivity::Signal,
    traits::{IntoRx, IntoSignal, RxValue},
};
use std::{
    fmt::{Display, Formatter, Result},
    marker::PhantomData,
    ops::{Add, Div, Mul, Sub},
};

mod calc;
mod colors;
mod complex;
mod gradients;
mod shorthands;
mod units;

use units::{for_all_angle_units, for_all_length_units, for_all_time_units};

pub use calc::*;
pub use colors::*;
pub use complex::*;
pub use gradients::*;
pub use shorthands::*;
pub use units::*;

/// 核心验证 Trait
/// 用于保证传入的值属于当前 CSS 属性合法的类型。
#[diagnostic::on_unimplemented(
    message = "类型 `{Self}` 无法作为有效的 CSS `{Prop}` 属性值使用",
    label = "无效的 CSS 属性类型",
    note = "请检查是否传入了错误的类型（例如将 Px 传给了 Color）。如果必须传入复杂的动态表达式，可以使用 `css_unsafe(...)` 显式绕过。"
)]
pub trait ValidFor<Prop> {}

pub trait CssValue: Display {}
impl<T: Display> CssValue for T {}

/// 可以「取消」的属性值。
///
/// `None` 渲染成 CSS 宽关键字 `unset`——即该属性回到继承值（可继承属性）或
/// 初始值（不可继承属性）。
///
/// 此前 `None` 渲染成**空串**，注释写的是「在 CSS 中不输出（实现响应式移除）」，
/// 而两条路的实际行为既不一致、也都不是那个意思：
///
/// - 静态路径产出 `prop: ;`——无效声明，浏览器丢弃，碰巧接近「不输出」；
/// - 动态路径产出 `prop: var(--x)` 且 `--x` 被设成空串，触发
///   *invalid at computed-value time*，属性取继承值或初始值。
///
/// 真正的「不输出」在动态路径上做不到（声明写在类规则里，能改的只有变量的值），
/// 所以两边统一到动态路径本来就会落到的那个语义上，并把它写清楚。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CssOption<T> {
    #[default]
    None,
    Some(T),
}

impl<T> CssOption<T> {
    pub const fn some(val: T) -> Self {
        Self::Some(val)
    }

    pub const fn none() -> Self {
        Self::None
    }
}

pub const fn css_some<T>(val: T) -> CssOption<T> {
    CssOption::Some(val)
}

pub const fn css_none<T>() -> CssOption<T> {
    CssOption::None
}

impl<T: Display> Display for CssOption<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Some(val) => write!(f, "{}", val),
            Self::None => write!(f, "unset"),
        }
    }
}

impl<T> From<Option<T>> for CssOption<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Option::Some(val) => Self::Some(val),
            Option::None => Self::None,
        }
    }
}

impl<T> From<CssOption<T>> for Option<T> {
    fn from(opt: CssOption<T>) -> Self {
        match opt {
            CssOption::Some(val) => Option::Some(val),
            CssOption::None => Option::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CssUnsafe(String);
impl CssUnsafe {
    pub fn new<T: Display>(val: T) -> Self {
        Self(val.to_string())
    }
}
pub fn css_unsafe<T: Display>(val: T) -> CssUnsafe {
    CssUnsafe(val.to_string())
}
impl Display for CssUnsafe {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CssVarValue {
    Static(&'static str),
    Dynamic(String),
}

impl PartialEq<&str> for CssVarValue {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Self::Static(s) => s == other,
            Self::Dynamic(s) => s == other,
        }
    }
}

impl Display for CssVarValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Static(s) => write!(f, "{}", s),
            Self::Dynamic(s) => write!(f, "{}", s),
        }
    }
}

/// CSS 变量类型，可通过 `css_var()` 函数创建。
/// 泛型 T 用于强类型校验，例如 `CssVar<Hex>` 仅在接收颜色的属性中有效。
#[derive(Clone, Debug, PartialEq)]
pub struct CssVar<T = ()>(pub CssVarValue, pub PhantomData<T>);

impl<T: CssColor> CssVar<T> {
    pub fn alpha(self, alpha: f64) -> Self {
        Self(
            CssVarValue::Dynamic(format!(
                "color-mix(in srgb, {}, transparent {}%)",
                self.0,
                (1.0 - alpha) * 100.0
            )),
            PhantomData,
        )
    }
}

impl<T> From<T> for CssVar<T>
where
    T: Display,
{
    fn from(val: T) -> Self {
        Self(CssVarValue::Dynamic(val.to_string()), PhantomData)
    }
}

impl<T> Default for CssVar<T> {
    fn default() -> Self {
        Self(CssVarValue::Static(""), PhantomData)
    }
}

impl<T> Display for CssVar<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.0)
    }
}

/// 创建一个 CSS 变量引用。
/// 如果输入不带 `var()` 前缀，会自动包裹。
/// 返回的 CssVar<()> 对所有 CSS 属性有效（不安全模式）。
pub fn css_var(name: impl Display) -> CssVar<()> {
    let name_str = name.to_string();
    let val = if name_str.starts_with("var(") {
        name_str
    } else {
        format!("var({})", name_str)
    };
    CssVar(CssVarValue::Dynamic(val), PhantomData)
}

// ==========================================
// 关键字 Enum 自动化
// ==========================================

#[macro_export]
macro_rules! define_css_enum {
    (ColorKeyword ($($prop:path),*) $rest:tt) => {
        define_css_enum!(@base ColorKeyword $rest);
        // We handle ColorKeyword's ValidFor impls manually in define_props! or via traits
    };
    ($name:ident ($($prop:path),*) { $($variant:ident => $val:expr),* $(,)? }) => {
        define_css_enum!(@base $name { $($variant => $val),* });
        $(impl ValidFor<$prop> for $name {})*
    };
    (@base $name:ident { $($variant:ident => $val:expr),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub enum $name { $($variant),* }
        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> Result {
                match self { $(Self::$variant => write!(f, $val)),* }
            }
        }
    };
}

pub use crate::codegen::keywords_gen::*;

// ==========================================
// 属性定义与基础约束自动化
// ==========================================

macro_rules! impl_css_ops {
    ($t:ty, $trait:ident, $mark:ident) => {
        impl<R: $trait + CalcOperand> Add<R> for $t {
            type Output = CalcValue<$mark>;
            fn add(self, rhs: R) -> Self::Output {
                CalcValue::binary(self, "+", rhs)
            }
        }
        impl<R: $trait + CalcOperand> Sub<R> for $t {
            type Output = CalcValue<$mark>;
            fn sub(self, rhs: R) -> Self::Output {
                CalcValue::binary(self, "-", rhs)
            }
        }
        impl Mul<f64> for $t {
            type Output = CalcValue<$mark>;
            fn mul(self, rhs: f64) -> Self::Output {
                CalcValue::binary(self, "*", rhs)
            }
        }
        impl Div<f64> for $t {
            type Output = CalcValue<$mark>;
            fn div(self, rhs: f64) -> Self::Output {
                CalcValue::binary(self, "/", rhs)
            }
        }
    };
}

/// 一个量纲家族：所有成员都实现列出的标记 trait，并获得四则运算符。
///
/// 运算符的右操作数被约束为该量纲的**操作数 trait**，所以 `px(1) + deg(1)`
/// 不成立——量纲标记要挡住的就是这个。长度这一族的操作数 trait 是
/// `CssLengthPercentage` 而不是 `CssLength`，因为 `calc(100% - 10px)` 合法。
macro_rules! impl_dimension_family {
    // 标记 trait 逐个消耗——`$trait` 与 `$t` 都是深度 1 的重复，嵌套写不出来
    ([], $operand:ident, $mark:ident: $($t:ty),* $(,)?) => {
        $( impl_css_ops!($t, $operand, $mark); )*
    };
    ([$first:ident $(, $rest:ident)* $(,)?], $operand:ident, $mark:ident: $($t:ty),* $(,)?) => {
        $( impl $first for $t {} )*
        impl_dimension_family!([$($rest),*], $operand, $mark: $($t),*);
    };
}
macro_rules! impl_length_family {
    ($($t:ty),* $(,)?) => {
        impl_dimension_family!(
            [CssLength, CssLengthPercentage], CssLengthPercentage, LengthMark: $($t),*
        );
    };
}
macro_rules! impl_angle_family {
    ($($t:ty),* $(,)?) => { impl_dimension_family!([CssAngle], CssAngle, AngleMark: $($t),*); };
}
macro_rules! impl_time_family {
    ($($t:ty),* $(,)?) => { impl_dimension_family!([CssTime], CssTime, TimeMark: $($t),*); };
}

for_all_length_units!(impl_length_family);
// 百分比与长度算式属于 `<length-percentage>` 而**不属于** `<length>`
impl_dimension_family!(
    [CssLengthPercentage], CssLengthPercentage, LengthMark: Percent, CalcValue<LengthMark>
);
for_all_angle_units!(impl_angle_family);
impl_angle_family!(CalcValue<AngleMark>);
for_all_time_units!(impl_time_family);
impl_time_family!(CalcValue<TimeMark>);

impl CssColor for Rgba {}
impl CssColor for Hex {}
impl CssColor for Hsl {}
impl CssColor for ColorFn {}
impl CssColor for ColorKeyword {}
impl CssColor for ColorName {}

pub trait CssProperty {
    type Value;
    const PROPERTY_NAME: &'static str;
}

/// 把注册表里的能力标记展开成 `ValidFor` 实现。
///
/// 能力集合来自对 MDN 值定义语法的真实解析（`silex_codegen::css::syntax`），
/// 每个能力对应的类型集合必须**互不重叠**——重叠会直接编译失败（E0119），
/// 所以 `Num` 覆盖整数、`Int` 只在没有 `Num` 时出现，`LenCalc` 只在有长度或
/// 百分比时出现。
macro_rules! define_props {
    ($( ($snake:ident, $kebab:expr, $pascal:ident, [$($cap:ident)*]) ),*) => {
        pub mod props {
            $( pub struct $pascal; )*
            pub struct Any;
        }

        $(
            impl $crate::types::CssProperty for props::$pascal {
                type Value = ();
                const PROPERTY_NAME: &'static str = $kebab;
            }
        )*

        impl $crate::types::CssProperty for props::Any {
            type Value = ();
            const PROPERTY_NAME: &'static str = "";
        }

        $(
            // 所有属性默认支持 CssUnsafe 和无类型限制的 CssVar<()>
            impl ValidFor<props::$pascal> for CssUnsafe {}
            impl ValidFor<props::$pascal> for CssVar<()> {}
            // CSS 宽关键字对**每一个**属性都合法，这是规范的一部分
            impl ValidFor<props::$pascal> for CssWide {}
            // 核心：强类型 CssVar<T> 继承 T 的校验规则
            impl<T> ValidFor<props::$pascal> for CssVar<T> where T: ValidFor<props::$pascal> {}
            // 支持 CssOption<T> 作为合法属性值类型，当为 None 时渲染成 `unset`
            impl<T> ValidFor<props::$pascal> for CssOption<T> where T: ValidFor<props::$pascal> {}

            $( define_props!(@cap $pascal, $cap); )*
        )*
    };
    // `<length>`：单位清单由 `units::for_all_length_units!` 展开，
    // 与 `CssLength` / 算术运算符 / `calc()` 操作数共用同一份事实
    (@cap $pascal:ident, Length) => {
        for_all_length_units!(define_props, @valid, $pascal);
    };
    // 所有具体值类型的 `ValidFor` 都从这里出去。
    //
    // `#[diagnostic::do_not_recommend]` 是为了错误信息：不加它，一条
    // `align_items(hex(…))` 会让 rustc 附上一张「`Hex` 还实现了
    // ValidFor<BackgroundColor> / ValidFor<Border> / …」的清单——40 多行，
    // 讲的全是**别的**属性，而用户的问题是「AlignItems 能收什么」。
    // trybuild 快照曾因此长到 455 行、其中 433 行是这张清单，人工无法审阅 diff。
    // `on_unimplemented` 里的定制说明已经把该说的说清楚了。
    (@valid, $pascal:ident, $($t:ty),* $(,)?) => {
        $(
            #[diagnostic::do_not_recommend]
            impl ValidFor<props::$pascal> for $t {}
        )*
    };
    // `<percentage>`
    (@cap $pascal:ident, Percent) => {
        define_props!(@valid, $pascal, Percent);
    };
    // 只要接受长度或百分比，就接受长度算式
    (@cap $pascal:ident, LenCalc) => {
        define_props!(@valid, $pascal, CalcValue<LengthMark>);
    };
    // `<time>`：此前 `<time>` 只被算作「需要裸字符串」，于是
    // `transition-duration` / `animation-delay` 只能写 `"0.3s"`
    (@cap $pascal:ident, Time) => {
        for_all_time_units!(define_props, @valid, $pascal);
        define_props!(@valid, $pascal, CalcValue<TimeMark>);
    };
    // `<flex>`：网格轨道的 `fr`，不与长度互通
    (@cap $pascal:ident, Flex) => {
        define_props!(@valid, $pascal, Fr);
    };
    // `<number>`：同时覆盖整数字面量
    (@cap $pascal:ident, Num) => {
        define_props!(@valid, $pascal, f64, f32);
        define_props!(@cap $pascal, Int);
    };
    // `<integer>`
    (@cap $pascal:ident, Int) => {
        define_props!(@valid, $pascal, i32, u32, i64, u64, isize, usize);
    };
    // `<angle>`
    (@cap $pascal:ident, Angle) => {
        for_all_angle_units!(define_props, @valid, $pascal);
        define_props!(@valid, $pascal, CalcValue<AngleMark>);
    };
    // `<color>`
    (@cap $pascal:ident, Color) => {
        define_props!(@valid, $pascal, Rgba, Hex, Hsl, ColorFn, ColorKeyword, ColorName);
    };
    // `<url>` / `<image>`
    (@cap $pascal:ident, Url) => {
        define_props!(@valid, $pascal, Url);
    };
    // 取值可能由多个分量拼成，或者含有 Rust 侧没有对应类型的东西
    // （`<custom-ident>`、解析不出来的引用）——只能写裸字符串
    (@cap $pascal:ident, Str) => {
        define_props!(@valid, $pascal, String, &'static str);
    };
}

// 调用中心注册表执行代码生成
for_all_properties!(define_props);

// --- 手动补充跨组约束 ---
//
// `border()` 产出的是 `<width> <style> <color>` 三段式，所以它只对**简写**属性
// 合法。这里曾经还有一条 `impl ValidFor<props::BorderColor> for BorderValue`
// ——`border_color(border(px(1), Solid, red))` 会产出
// `border-color: 1px solid red;`，浏览器整条丢弃。与 P3-2 的 `margin::top`
// 是同一类问题：名字对得上，语义对不上。
impl ValidFor<props::Border> for BorderValue {}
impl ValidFor<props::BorderTop> for BorderValue {}
impl ValidFor<props::BorderRight> for BorderValue {}
impl ValidFor<props::BorderBottom> for BorderValue {}
impl ValidFor<props::BorderLeft> for BorderValue {}
impl ValidFor<props::BorderInlineStart> for BorderValue {}
impl ValidFor<props::BorderInlineEnd> for BorderValue {}
impl ValidFor<props::BorderBlockStart> for BorderValue {}
impl ValidFor<props::BorderBlockEnd> for BorderValue {}
// `Url` 对 `background` / `background-image` 的合法性现在由注册表的 `Url`
// 能力自动生成（这两个属性的语法里确实有 `<image>`），不再手写。
impl<T: Display> ValidFor<props::Any> for T {}

// 响应式集成后的注册
macro_rules! impl_into_rx_for_css {
    ($($t:ty),*) => {
        $(
            impl RxValue for $t {
                type Value = $t;
            }

            impl IntoRx for $t {
                type RxType = Rx<$t, RxValueKind>;
                fn into_rx(self) -> Self::RxType {
                    Rx::new_constant(self)
                }
                fn is_constant(&self) -> bool { true }
            }

            impl IntoSignal for $t {
                fn into_signal(self) -> Signal<$t>
                where
                    Self: Sized + 'static,
                    $t: Sized + Clone + 'static,
                {
                    Signal::from(self)
                }
            }
        )*
    };
}

impl<T: 'static> RxValue for CssVar<T> {
    type Value = Self;
}
impl<T: 'static> IntoRx for CssVar<T> {
    type RxType = Rx<Self, RxValueKind>;
    fn into_rx(self) -> Self::RxType {
        Rx::new_constant(self)
    }
    fn is_constant(&self) -> bool {
        true
    }
}
impl<T: Clone + 'static> IntoSignal for CssVar<T> {
    fn into_signal(self) -> Signal<Self> {
        Signal::from(self)
    }
}

impl<T: Display + Clone + 'static> RxValue for CssOption<T> {
    type Value = Self;
}
impl<T: Display + Clone + 'static> IntoRx for CssOption<T> {
    type RxType = Rx<Self, RxValueKind>;
    fn into_rx(self) -> Self::RxType {
        Rx::new_constant(self)
    }
    fn is_constant(&self) -> bool {
        true
    }
}
impl<T: Display + Clone + 'static> IntoSignal for CssOption<T> {
    fn into_signal(self) -> Signal<Self> {
        Signal::from(self)
    }
}

for_all_length_units!(impl_into_rx_for_css);
for_all_angle_units!(impl_into_rx_for_css);
for_all_time_units!(impl_into_rx_for_css);

impl_into_rx_for_css!(
    Percent,
    Fr,
    Rgba,
    Auto,
    Hex,
    Hsl,
    ColorFn,
    ColorName,
    NoneValue,
    CssWide,
    Url,
    BorderValue,
    MarginValue,
    PaddingValue,
    FlexValue,
    TransitionValue,
    BackgroundValue,
    CssUnsafe,
    TransformValue,
    TransformBuilder,
    GridTemplateAreasValue,
    FontVariationSettingsValue,
    CalcValue<LengthMark>,
    CalcValue<AngleMark>,
    CalcValue<TimeMark>,
    GradientValue
);

crate::register_generated_keywords!(impl_into_rx_for_css);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::Style;

    /// 规范里这五个词对任何属性都合法。此前 `inherit` 在 361 个关键字枚举里
    /// 只出现过 2 次，`sty().color("inherit")` 直接编译失败。
    #[test]
    fn css_wide_keywords_render_themselves() {
        assert_eq!(INHERIT.to_string(), "inherit");
        assert_eq!(INITIAL.to_string(), "initial");
        assert_eq!(UNSET.to_string(), "unset");
        assert_eq!(REVERT.to_string(), "revert");
        assert_eq!(REVERT_LAYER.to_string(), "revert-layer");
    }

    /// 编译得过就说明 `CssWide` 对这几类互不相干的属性都有效
    #[test]
    fn css_wide_is_valid_for_every_kind_of_property() {
        let _ = Style::new()
            .color(INHERIT)
            .align_items(INITIAL)
            .z_index(UNSET)
            .width(REVERT)
            .transform(REVERT_LAYER);
    }

    /// 关键字集合相同的属性现在共用同一个枚举：
    /// `AlignItemsKeyword::Center` 与 `PlaceItemsKeyword::Center` 是同一个值
    #[test]
    fn properties_with_the_same_keyword_set_share_one_enum() {
        let a: AlignItemsKeyword = AlignItemsKeyword::Center;
        let b: PlaceItemsKeyword = PlaceItemsKeyword::Center;
        assert_eq!(a, b);
    }

    /// 纯 `auto` / 纯 `none` 的属性直接复用全局类型，不再各生成一个枚举
    #[test]
    fn bare_auto_and_none_reuse_the_global_types() {
        let _ = Style::new().width(AUTO).transform(NONE);
        assert_eq!(AUTO.to_string(), "auto");
        assert_eq!(NONE.to_string(), "none");
    }

    /// 报告 P2-11：`None` 此前渲染成空串，静态路径产出 `prop: ;`、动态路径
    /// 产出 `prop: var(--x)` 且 `--x` 为空，两条路行为不同，也都不等于注释
    /// 里写的「不输出」。现在统一成 `unset`，两条路一致且语义明确。
    #[test]
    fn css_option_none_renders_as_unset() {
        assert_eq!(css_none::<Px>().to_string(), "unset");
        assert_eq!(css_some(px(4)).to_string(), "4px");
        assert_eq!(CssOption::<Hex>::default().to_string(), "unset");
    }

    /// 静态路径不再产出 `width: ;` 这种无效声明
    #[test]
    fn css_option_none_produces_a_valid_declaration() {
        let css = Style::new().width(css_none::<Px>()).render().css;
        assert!(css.contains("width: unset;"), "{css}");
    }

    /// 值类型现在按属性的实际语法约束，而不是一刀切
    #[test]
    fn typed_values_land_on_the_right_properties() {
        let _ = Style::new()
            .width(px(10))
            .color(hex("#fff"))
            .opacity(0.5)
            .z_index(3)
            .rotate(deg(90))
            .background_image(url("a.png"))
            // 真正的复合属性仍然收裸字符串
            .transition("all 0.3s")
            .margin("0 auto");
    }

    /// 报告 P3-8：没有时间单位，`transition_duration` / `animation_delay`
    /// 只能吃字符串
    #[test]
    fn time_units_land_on_time_properties() {
        let css = Style::new()
            .transition_duration(sec(0.3))
            .animation_delay(ms(150))
            .render()
            .css;
        assert!(css.contains("transition-duration: 0.3s;"), "{css}");
        assert!(css.contains("animation-delay: 150ms;"), "{css}");
    }

    /// `fr` 只在网格轨道尺寸里合法
    #[test]
    fn fr_lands_on_grid_track_properties() {
        let css = Style::new().grid_auto_columns(fr(1)).render().css;
        assert!(css.contains("grid-auto-columns: 1fr;"), "{css}");
    }

    /// 现代颜色语法能用在任何接受 `<color>` 的属性上
    #[test]
    fn modern_color_functions_land_on_color_properties() {
        let css = Style::new()
            .color(oklch(0.7, 0.15, 250))
            .background_color(color_mix(ColorSpace::Oklch, hex("#fff"), hex("#000")))
            .render()
            .css;
        assert!(css.contains("color: oklch(0.7 0.15 250);"), "{css}");
        assert!(css.contains("color-mix(in oklch, #fff, #000)"), "{css}");
    }

    /// `for_all_length_units!` 是长度量纲的唯一事实来源：这条测试同时钉住
    /// `ValidFor<接受 <length> 的属性>`（`width`）与 `CssLengthPercentage`
    /// （算术运算符）两份展开——任何一份漏掉某个单位都会在这里编译失败
    #[test]
    fn every_length_unit_reaches_both_the_property_table_and_the_operators() {
        macro_rules! check {
            ($($t:ident),* $(,)?) => {$(
                let v = $t::from(1);
                let _ = Style::new().width(v);
                let _ = v + px(1);
                let _ = px(1) + v;
            )*};
        }
        for_all_length_units!(check);
        let _ = Style::new().width(pct(50));
        let _ = pct(50) + px(1);
    }

    /// 角度与时间同理
    #[test]
    fn every_angle_and_time_unit_reaches_its_properties() {
        macro_rules! check_angle {
            ($($t:ident),* $(,)?) => {$(
                let v = $t::from(1);
                let _ = Style::new().rotate(v);
                let _ = v + deg(1);
            )*};
        }
        for_all_angle_units!(check_angle);

        macro_rules! check_time {
            ($($t:ident),* $(,)?) => {$(
                let v = $t::from(1);
                let _ = Style::new().transition_duration(v);
                let _ = v + sec(1);
            )*};
        }
        for_all_time_units!(check_time);
    }
}
