use std::fmt::Display;

/// 核心验证 Trait
/// 用于保证传入的值属于当前 CSS 属性合法的类型。
pub trait ValidFor<Prop> {}

/// 所有具有单位的封装体都应当去实现的基本展示 Traits 别名组合
pub trait CssValue: Display {}

impl<T: Display> CssValue for T {}

/// 属性标记，由宏在解析期提取并映射
pub mod props {
    pub struct Width;
    pub struct Height;
    pub struct Margin;
    pub struct Padding;
    pub struct Color;
    pub struct BackgroundColor;
    pub struct ZIndex;
    pub struct Any; // 用于未识别的情况，做平滑降级
}

// ==========================================
// 核心包裹单元类型 (Units)
// ==========================================

#[derive(Clone, Copy, Debug)]
pub struct Px(pub f64);

impl Display for Px {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}px", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Percent(pub f64);

impl Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rgba(pub u8, pub u8, pub u8, pub f32);

impl Display for Rgba {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rgba({}, {}, {}, {})", self.0, self.1, self.2, self.3)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Auto;

impl Display for Auto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "auto")
    }
}

// 快速包裹函数
#[inline]
pub fn px<T: Into<f64>>(v: T) -> Px {
    Px(v.into())
}

#[inline]
pub fn pct<T: Into<f64>>(v: T) -> Percent {
    Percent(v.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Rem(pub f64);

impl Display for Rem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}rem", self.0)
    }
}

#[inline]
pub fn rem<T: Into<f64>>(v: T) -> Rem {
    Rem(v.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Em(pub f64);

impl Display for Em {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}em", self.0)
    }
}

#[inline]
pub fn em<T: Into<f64>>(v: T) -> Em {
    Em(v.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Vw(pub f64);

impl Display for Vw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}vw", self.0)
    }
}

#[inline]
pub fn vw<T: Into<f64>>(v: T) -> Vw {
    Vw(v.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Vh(pub f64);

impl Display for Vh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}vh", self.0)
    }
}

#[inline]
pub fn vh<T: Into<f64>>(v: T) -> Vh {
    Vh(v.into())
}

#[inline]
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    Rgba(r, g, b, a)
}

// ==========================================
// 属性与类型绑定约束实施 (Traits Impl)
// ==========================================

macro_rules! impl_valid_for_dimension {
    ($prop:ty) => {
        impl ValidFor<$prop> for Px {}
        impl ValidFor<$prop> for Percent {}
        impl ValidFor<$prop> for Rem {}
        impl ValidFor<$prop> for Em {}
        impl ValidFor<$prop> for Vw {}
        impl ValidFor<$prop> for Vh {}
        impl ValidFor<$prop> for Auto {}
    };
}

impl_valid_for_dimension!(props::Width);
impl_valid_for_dimension!(props::Height);
impl_valid_for_dimension!(props::Margin);
impl_valid_for_dimension!(props::Padding);

impl ValidFor<props::ZIndex> for i32 {}
impl ValidFor<props::ZIndex> for u32 {}
impl ValidFor<props::ZIndex> for i64 {}
impl ValidFor<props::ZIndex> for u64 {}
impl ValidFor<props::ZIndex> for isize {}
impl ValidFor<props::ZIndex> for usize {}

impl ValidFor<props::Color> for Rgba {}

impl ValidFor<props::BackgroundColor> for Rgba {}

// Any 现在只用来作为“any”上下文（比如动态 selector 的变量），不再有显示回退
impl<T: Display> ValidFor<props::Any> for T {}

/// 显示地放弃强类型检查，用于目前类型系统尚未支持的复杂 CSS 值，如 `calc(...)`。
#[derive(Clone, Debug)]
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

impl ValidFor<props::Width> for UnsafeCss {}
impl ValidFor<props::Height> for UnsafeCss {}
impl ValidFor<props::Margin> for UnsafeCss {}
impl ValidFor<props::Padding> for UnsafeCss {}
impl ValidFor<props::ZIndex> for UnsafeCss {}
impl ValidFor<props::Color> for UnsafeCss {}
impl ValidFor<props::BackgroundColor> for UnsafeCss {}
