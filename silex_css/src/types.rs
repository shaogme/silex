use std::fmt::Display;

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
    note = "请检查是否传入了错误的类型（例如将 Px 传给了 Color）。如果必须传入复杂的动态表达式，可以使用 `UnsafeCss::new(...)` 显式绕过。"
)]
pub trait ValidFor<Prop> {}

pub trait CssValue: Display {}
impl<T: Display> CssValue for T {}

#[derive(Clone, Debug, PartialEq)]
pub struct UnsafeCss(pub String);
impl UnsafeCss {
    pub fn new<T: Display>(val: T) -> Self {
        Self(val.to_string())
    }
}
impl Display for UnsafeCss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ==========================================
// 关键字 Enum 自动化
// ==========================================

macro_rules! define_css_enum {
    ($name:ident ($($prop:path),*) { $($variant:ident => $val:expr),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub enum $name { $($variant),* }
        impl Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self { $(Self::$variant => write!(f, $val)),* }
            }
        }
        $(impl crate::types::ValidFor<$prop> for $name {})*
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
        impl<R: $trait> std::ops::Add<R> for $t {
            type Output = CalcValue<$mark>;
            fn add(self, rhs: R) -> Self::Output {
                CalcValue::binary(self, " + ", rhs)
            }
        }
        impl<R: $trait> std::ops::Sub<R> for $t {
            type Output = CalcValue<$mark>;
            fn sub(self, rhs: R) -> Self::Output {
                CalcValue::binary(self, " - ", rhs)
            }
        }
        impl std::ops::Mul<f64> for $t {
            type Output = CalcValue<$mark>;
            fn mul(self, rhs: f64) -> Self::Output {
                CalcValue::binary(self, " * ", rhs)
            }
        }
        impl std::ops::Div<f64> for $t {
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

macro_rules! define_props {
    ($( ($snake:ident, $kebab:expr, $pascal:ident, $group:ident) ),*) => {
        pub mod props {
            $( pub struct $pascal; )*
            pub struct Any;
        }

        // 所有属性默认支持 UnsafeCss
        $( impl ValidFor<props::$pascal> for UnsafeCss {} )*

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
    };
    // 复合属性专用 (如 border, margin)
    (@group $pascal:ident, Shorthand) => {
        impl ValidFor<props::$pascal> for String {}
        impl ValidFor<props::$pascal> for &'static str {}
        impl_valid_for_dimension!(props::$pascal);
        impl ValidFor<props::$pascal> for Rgba {}
        impl ValidFor<props::$pascal> for Hex {}
        impl ValidFor<props::$pascal> for Hsl {}
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
crate::for_all_properties!(define_props);

// --- 手动补充跨组约束 ---
impl ValidFor<props::Border> for BorderValue {}
impl ValidFor<props::Background> for Url {}
impl ValidFor<props::BackgroundImage> for Url {}
impl<T: Display> ValidFor<props::Any> for T {}

// 响应式集成后的注册
macro_rules! impl_into_rx_for_css {
    ($($t:ty),*) => {
        $(
            impl silex_core::traits::IntoRx for $t {
                type Value = $t;
                type RxType = silex_core::Rx<silex_core::reactivity::Constant<$t>, silex_core::RxValue>;
                fn into_rx(self) -> Self::RxType { silex_core::Rx(silex_core::reactivity::Constant(self), ::core::marker::PhantomData) }
                fn is_constant(&self) -> bool { true }
                fn into_signal(self) -> silex_core::reactivity::Signal<$t>
                where
                    Self: Sized + 'static,
                    $t: Sized + Clone + 'static,
                {
                    silex_core::reactivity::Signal::derive(move || self.clone())
                }
            }
        )*
    };
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
    Url,
    BorderValue,
    MarginValue,
    PaddingValue,
    FlexValue,
    TransitionValue,
    BackgroundValue,
    UnsafeCss,
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
