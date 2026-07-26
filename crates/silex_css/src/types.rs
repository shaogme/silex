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
mod complex;
mod gradients;
mod shorthands;
mod units;

pub use calc::*;
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
            Self::None => Ok(()),
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

impl CssLength for Px {}
impl CssLength for Percent {}
impl CssLength for Rem {}
impl CssLength for Em {}
impl CssLength for Vw {}
impl CssLength for Vh {}

impl CssAngle for Deg {}
impl CssAngle for Rad {}
impl CssAngle for Turn {}

impl CssColor for Rgba {}
impl CssColor for Hex {}
impl CssColor for Hsl {}
impl CssColor for ColorKeyword {}

impl_css_ops!(Px, CssLength, LengthMark);
impl_css_ops!(Percent, CssLength, LengthMark);
impl_css_ops!(Rem, CssLength, LengthMark);
impl_css_ops!(Em, CssLength, LengthMark);
impl_css_ops!(Vw, CssLength, LengthMark);
impl_css_ops!(Vh, CssLength, LengthMark);
impl_css_ops!(CalcValue<LengthMark>, CssLength, LengthMark);

impl_css_ops!(Deg, CssAngle, AngleMark);
impl_css_ops!(Rad, CssAngle, AngleMark);
impl_css_ops!(Turn, CssAngle, AngleMark);
impl_css_ops!(CalcValue<AngleMark>, CssAngle, AngleMark);

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
            // 支持 CssOption<T> 作为合法属性值类型，当为 None 时在 CSS 中不输出（实现响应式移除）
            impl<T> ValidFor<props::$pascal> for CssOption<T> where T: ValidFor<props::$pascal> {}

            $( define_props!(@cap $pascal, $cap); )*
        )*
    };
    // `<length>`
    (@cap $pascal:ident, Length) => {
        impl ValidFor<props::$pascal> for Px {}
        impl ValidFor<props::$pascal> for Rem {}
        impl ValidFor<props::$pascal> for Em {}
        impl ValidFor<props::$pascal> for Vw {}
        impl ValidFor<props::$pascal> for Vh {}
    };
    // `<percentage>`
    (@cap $pascal:ident, Percent) => {
        impl ValidFor<props::$pascal> for Percent {}
    };
    // 只要接受长度或百分比，就接受长度算式
    (@cap $pascal:ident, LenCalc) => {
        impl ValidFor<props::$pascal> for CalcValue<LengthMark> {}
    };
    // `<number>`：同时覆盖整数字面量
    (@cap $pascal:ident, Num) => {
        impl ValidFor<props::$pascal> for f64 {}
        impl ValidFor<props::$pascal> for f32 {}
        define_props!(@cap $pascal, Int);
    };
    // `<integer>`
    (@cap $pascal:ident, Int) => {
        impl ValidFor<props::$pascal> for i32 {}
        impl ValidFor<props::$pascal> for u32 {}
        impl ValidFor<props::$pascal> for i64 {}
        impl ValidFor<props::$pascal> for u64 {}
        impl ValidFor<props::$pascal> for isize {}
        impl ValidFor<props::$pascal> for usize {}
    };
    // `<angle>`
    (@cap $pascal:ident, Angle) => {
        impl ValidFor<props::$pascal> for Deg {}
        impl ValidFor<props::$pascal> for Rad {}
        impl ValidFor<props::$pascal> for Turn {}
        impl ValidFor<props::$pascal> for CalcValue<AngleMark> {}
    };
    // `<color>`
    (@cap $pascal:ident, Color) => {
        impl ValidFor<props::$pascal> for Rgba {}
        impl ValidFor<props::$pascal> for Hex {}
        impl ValidFor<props::$pascal> for Hsl {}
        impl ValidFor<props::$pascal> for ColorKeyword {}
    };
    // `<url>` / `<image>`
    (@cap $pascal:ident, Url) => {
        impl ValidFor<props::$pascal> for Url {}
    };
    // 取值可能由多个分量拼成，或者含有 Rust 侧没有对应类型的东西
    // （`<custom-ident>`、`<time>`、解析不出来的引用）——只能写裸字符串
    (@cap $pascal:ident, Str) => {
        impl ValidFor<props::$pascal> for String {}
        impl ValidFor<props::$pascal> for &'static str {}
    };
}

// 调用中心注册表执行代码生成
for_all_properties!(define_props);

// --- 手动补充跨组约束 ---
impl ValidFor<props::Border> for BorderValue {}
impl ValidFor<props::BorderColor> for BorderValue {}
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

impl_into_rx_for_css!(
    Px,
    Percent,
    Rgba,
    Auto,
    Rem,
    Em,
    Vw,
    Vh,
    Hex,
    Hsl,
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
    Deg,
    Rad,
    Turn,
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
}
