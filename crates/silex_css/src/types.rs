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

include!("keywords_gen.rs");

// ==========================================
// 属性定义与基础约束自动化
// ==========================================

macro_rules! impl_valid_for_dimension {
    ($prop:ty) => {
        impl ValidFor<$prop> for Px {}
        impl ValidFor<$prop> for Percent {}
        impl ValidFor<$prop> for Rem {}
        impl ValidFor<$prop> for Em {}
        impl ValidFor<$prop> for Vw {}
        impl ValidFor<$prop> for Vh {}
        impl ValidFor<$prop> for CalcValue<LengthMark> {}
    };
}

macro_rules! impl_css_ops {
    ($t:ty, $trait:ident, $mark:ident) => {
        impl<R: $trait> Add<R> for $t {
            type Output = CalcValue<$mark>;
            fn add(self, rhs: R) -> Self::Output {
                CalcValue::binary(self, " + ", rhs)
            }
        }
        impl<R: $trait> Sub<R> for $t {
            type Output = CalcValue<$mark>;
            fn sub(self, rhs: R) -> Self::Output {
                CalcValue::binary(self, " - ", rhs)
            }
        }
        impl Mul<f64> for $t {
            type Output = CalcValue<$mark>;
            fn mul(self, rhs: f64) -> Self::Output {
                CalcValue::binary(self, " * ", rhs)
            }
        }
        impl Div<f64> for $t {
            type Output = CalcValue<$mark>;
            fn div(self, rhs: f64) -> Self::Output {
                CalcValue::binary(self, " / ", rhs)
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

macro_rules! define_props {
    ($( ($snake:ident, $kebab:expr, $pascal:ident, $group:ident) ),*) => {
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

        // 所有属性默认支持 CssUnsafe 和无类型限制的 CssVar<()>
        $(
            impl ValidFor<props::$pascal> for CssUnsafe {}
            impl ValidFor<props::$pascal> for CssVar<()> {}
            // 核心：强类型 CssVar<T> 继承 T 的校验规则
            impl<T> ValidFor<props::$pascal> for CssVar<T> where T: ValidFor<props::$pascal> {}
            // 支持 CssOption<T> 作为合法属性值类型，当为 None 时在 CSS 中不输出（实现响应式移除）
            impl<T> ValidFor<props::$pascal> for CssOption<T> where T: ValidFor<props::$pascal> {}
        )*

        $(
            define_props!(@group $pascal, $group);
        )*
    };
    // 维度分组 (px, rem, vh 等)
    (@group $pascal:ident, Dimension) => {
        impl_valid_for_dimension!(props::$pascal);
    };
    // 颜色分组 (rgba, hex, hsl)
    (@group $pascal:ident, Color) => {
        impl ValidFor<props::$pascal> for Rgba {}
        impl ValidFor<props::$pascal> for Hex {}
        impl ValidFor<props::$pascal> for Hsl {}
        impl ValidFor<props::$pascal> for ColorKeyword {}
    };
    // 数字分组 (z-index, opacity 等)
    (@group $pascal:ident, Number) => {
        impl ValidFor<props::$pascal> for i32 {}
        impl ValidFor<props::$pascal> for u32 {}
        impl ValidFor<props::$pascal> for i64 {}
        impl ValidFor<props::$pascal> for u64 {}
        impl ValidFor<props::$pascal> for isize {}
        impl ValidFor<props::$pascal> for usize {}
        impl ValidFor<props::$pascal> for f64 {}
        impl ValidFor<props::$pascal> for f32 {}
    };
    // 复杂/自定义分组 (background, border, transform)
    (@group $pascal:ident, Custom) => {
        impl ValidFor<props::$pascal> for String {}
        impl ValidFor<props::$pascal> for &'static str {}
        impl_valid_for_dimension!(props::$pascal);
        impl ValidFor<props::$pascal> for Rgba {}
        impl ValidFor<props::$pascal> for Hex {}
        impl ValidFor<props::$pascal> for Hsl {}
        impl ValidFor<props::$pascal> for ColorKeyword {}
        impl ValidFor<props::$pascal> for NoneValue {}
    };
    // 复合属性专用 (如 border, margin)
    (@group $pascal:ident, Shorthand) => {
        impl ValidFor<props::$pascal> for String {}
        impl ValidFor<props::$pascal> for &'static str {}
        impl_valid_for_dimension!(props::$pascal);
        impl ValidFor<props::$pascal> for Rgba {}
        impl ValidFor<props::$pascal> for Hex {}
        impl ValidFor<props::$pascal> for Hsl {}
        impl ValidFor<props::$pascal> for ColorKeyword {}
        impl ValidFor<props::$pascal> for NoneValue {}
        impl ValidFor<props::$pascal> for i32 {}
        impl ValidFor<props::$pascal> for f64 {}
    };
    // 关键字分组 (由 define_css_enum 补充)
    (@group $pascal:ident, Keyword) => {};
    // 复杂属性分组 (transform, grid-template-areas 等)
    (@group $pascal:ident, Complex) => {
        impl ValidFor<props::$pascal> for String {}
        impl ValidFor<props::$pascal> for &'static str {}
    };
    // 透明度分组
    (@group $pascal:ident, Alpha) => {
        impl ValidFor<props::$pascal> for f64 {}
        impl ValidFor<props::$pascal> for f32 {}
        impl ValidFor<props::$pascal> for Percent {}
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
impl ValidFor<props::Background> for Url {}
impl ValidFor<props::BackgroundImage> for Url {}
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

register_generated_keywords!(impl_into_rx_for_css);
