use std::fmt::Display;

/// 核心验证 Trait
/// 用于保证传入的值属于当前 CSS 属性合法的类型。
#[diagnostic::on_unimplemented(
    message = "类型 `{Self}` 无法作为有效的 CSS `{Prop}` 属性值使用",
    label = "无效的 CSS 属性类型",
    note = "请检查是否传入了错误的类型（例如将 Px 传给了 Color）。如果必须传入复杂的动态表达式，可以使用 `UnsafeCss::new(...)` 显式绕过。"
)]
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
    pub struct Display;
    pub struct Position;
    pub struct FlexDirection;
    pub struct BackgroundImage;
    pub struct Any;

    // --- 复合与拆分属性 (Shorthands & Sub-properties) ---
    pub struct Border;
    pub struct BorderWidth;
    pub struct BorderStyle;
    pub struct BorderColor;
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
impl_valid_for_dimension!(props::BorderWidth);

impl ValidFor<props::ZIndex> for i32 {}
impl ValidFor<props::ZIndex> for u32 {}
impl ValidFor<props::ZIndex> for i64 {}
impl ValidFor<props::ZIndex> for u64 {}
impl ValidFor<props::ZIndex> for isize {}
impl ValidFor<props::ZIndex> for usize {}

impl ValidFor<props::Color> for Rgba {}

impl ValidFor<props::BackgroundColor> for Rgba {}
impl ValidFor<props::BorderColor> for Rgba {}

#[derive(Clone, Copy, Debug)]
pub enum DisplayKeyword {
    Flex,
    Block,
    Grid,
    None,
}

impl Display for DisplayKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flex => write!(f, "flex"),
            Self::Block => write!(f, "block"),
            Self::Grid => write!(f, "grid"),
            Self::None => write!(f, "none"),
        }
    }
}
impl ValidFor<props::Display> for DisplayKeyword {}

#[derive(Clone, Copy, Debug)]
pub enum PositionKeyword {
    Relative,
    Absolute,
    Fixed,
    Static,
    Sticky,
}

impl Display for PositionKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Relative => write!(f, "relative"),
            Self::Absolute => write!(f, "absolute"),
            Self::Fixed => write!(f, "fixed"),
            Self::Static => write!(f, "static"),
            Self::Sticky => write!(f, "sticky"),
        }
    }
}
impl ValidFor<props::Position> for PositionKeyword {}

#[derive(Clone, Copy, Debug)]
pub enum BorderStyleKeyword {
    None,
    Hidden,
    Dotted,
    Dashed,
    Solid,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

impl Display for BorderStyleKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::Hidden => write!(f, "hidden"),
            Self::Dotted => write!(f, "dotted"),
            Self::Dashed => write!(f, "dashed"),
            Self::Solid => write!(f, "solid"),
            Self::Double => write!(f, "double"),
            Self::Groove => write!(f, "groove"),
            Self::Ridge => write!(f, "ridge"),
            Self::Inset => write!(f, "inset"),
            Self::Outset => write!(f, "outset"),
        }
    }
}
impl ValidFor<props::BorderStyle> for BorderStyleKeyword {}

#[derive(Clone, Copy, Debug)]
pub enum FlexDirectionKeyword {
    Row,
    RowReverse,
    Column,
    ColumnReverse,
}

impl Display for FlexDirectionKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Row => write!(f, "row"),
            Self::RowReverse => write!(f, "row-reverse"),
            Self::Column => write!(f, "column"),
            Self::ColumnReverse => write!(f, "column-reverse"),
        }
    }
}
impl ValidFor<props::FlexDirection> for FlexDirectionKeyword {}

#[derive(Clone, Debug)]
pub struct Hex(pub String);

impl Display for Hex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[inline]
pub fn hex<T: Into<String>>(v: T) -> Hex {
    Hex(v.into())
}
impl ValidFor<props::Color> for Hex {}
impl ValidFor<props::BackgroundColor> for Hex {}
impl ValidFor<props::BorderColor> for Hex {}

#[derive(Clone, Copy, Debug)]
pub struct Hsl(pub u16, pub u8, pub u8);

impl Display for Hsl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hsl({}, {}%, {}%)", self.0, self.1, self.2)
    }
}

#[inline]
pub fn hsl(h: u16, s: u8, l: u8) -> Hsl {
    Hsl(h, s, l)
}
impl ValidFor<props::Color> for Hsl {}
impl ValidFor<props::BackgroundColor> for Hsl {}
impl ValidFor<props::BorderColor> for Hsl {}

#[derive(Clone, Debug)]
pub struct Url(pub String);

impl Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "url('{}')", self.0)
    }
}

#[inline]
pub fn url<T: Into<String>>(v: T) -> Url {
    Url(v.into())
}
impl ValidFor<props::BackgroundImage> for Url {}

// ==========================================
// 复合属性工厂 (Shorthand Factories)
// ==========================================

#[derive(Clone, Debug)]
pub struct BorderValue(pub String);

impl Display for BorderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ValidFor<props::Border> for BorderValue {}

/// 边框复合属性工厂函数
pub fn border<W, S, C>(width: W, style: S, color: C) -> BorderValue
where
    W: ValidFor<props::BorderWidth> + Display,
    S: ValidFor<props::BorderStyle> + Display,
    C: ValidFor<props::BorderColor> + Display,
{
    BorderValue(format!("{} {} {}", width, style, color))
}

pub mod margin {
    use super::*;
    pub fn all<T: ValidFor<props::Margin> + Display>(v: T) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn x_y<X, Y>(x: X, y: Y) -> UnsafeCss
    where
        X: ValidFor<props::Margin> + Display,
        Y: ValidFor<props::Margin> + Display,
    {
        UnsafeCss::new(format!("{} {}", x, y))
    }
    pub fn top_right_bottom_left<T, R, B, L>(top: T, right: R, bottom: B, left: L) -> UnsafeCss
    where
        T: ValidFor<props::Margin> + Display,
        R: ValidFor<props::Margin> + Display,
        B: ValidFor<props::Margin> + Display,
        L: ValidFor<props::Margin> + Display,
    {
        UnsafeCss::new(format!("{} {} {} {}", top, right, bottom, left))
    }
}

pub mod padding {
    use super::*;
    pub fn all<T: ValidFor<props::Padding> + Display>(v: T) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn x_y<X, Y>(x: X, y: Y) -> UnsafeCss
    where
        X: ValidFor<props::Padding> + Display,
        Y: ValidFor<props::Padding> + Display,
    {
        UnsafeCss::new(format!("{} {}", x, y))
    }
    pub fn top_right_bottom_left<T, R, B, L>(top: T, right: R, bottom: B, left: L) -> UnsafeCss
    where
        T: ValidFor<props::Padding> + Display,
        R: ValidFor<props::Padding> + Display,
        B: ValidFor<props::Padding> + Display,
        L: ValidFor<props::Padding> + Display,
    {
        UnsafeCss::new(format!("{} {} {} {}", top, right, bottom, left))
    }
}

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
impl ValidFor<props::Display> for UnsafeCss {}
impl ValidFor<props::Position> for UnsafeCss {}
impl ValidFor<props::FlexDirection> for UnsafeCss {}
impl ValidFor<props::BackgroundImage> for UnsafeCss {}
impl ValidFor<props::Border> for UnsafeCss {}
impl ValidFor<props::BorderWidth> for UnsafeCss {}
impl ValidFor<props::BorderStyle> for UnsafeCss {}
impl ValidFor<props::BorderColor> for UnsafeCss {}

// ==========================================
// 响应式信号集成 (Reactivity Integration)
// ==========================================

macro_rules! impl_into_signal_for_css {
    ($($t:ty),*) => {
        $(
            impl silex_core::traits::IntoSignal for $t {
                type Value = $t;
                type Signal = silex_core::reactivity::Constant<$t>;

                fn into_signal(self) -> Self::Signal {
                    silex_core::reactivity::Constant(self)
                }
            }
        )*
    };
}

impl_into_signal_for_css!(
    Px,
    Percent,
    Rgba,
    Auto,
    Rem,
    Em,
    Vw,
    Vh,
    DisplayKeyword,
    PositionKeyword,
    BorderStyleKeyword,
    FlexDirectionKeyword,
    Hex,
    Hsl,
    Url,
    BorderValue,
    UnsafeCss
);
