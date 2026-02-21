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

    pub struct BorderRadius;
    pub struct FontSize;
    pub struct FontWeight;
    pub struct LetterSpacing;
    pub struct LineHeight;
    pub struct TextAlign;
    pub struct TextDecoration;

    pub struct Cursor;
    pub struct Gap;

    pub struct AlignItems;
    pub struct JustifyContent;
    pub struct FlexWrap;
    pub struct FlexGrow;
    pub struct FlexShrink;
    pub struct FlexBasis;

    pub struct Top;
    pub struct Left;
    pub struct Right;
    pub struct Bottom;

    pub struct Opacity;
    pub struct Visibility;
    pub struct PointerEvents;

    pub struct Overflow;
    pub struct OverflowX;
    pub struct OverflowY;

    pub struct Transition;
    pub struct Transform;
    pub struct BoxShadow;
    pub struct BackdropFilter;
    pub struct Filter;

    pub struct Background;
    pub struct Outline;
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
impl_valid_for_dimension!(props::BorderRadius);
impl_valid_for_dimension!(props::FontSize);
impl_valid_for_dimension!(props::LetterSpacing);
impl_valid_for_dimension!(props::LineHeight);
impl_valid_for_dimension!(props::Gap);
impl_valid_for_dimension!(props::FlexBasis);
impl_valid_for_dimension!(props::Top);
impl_valid_for_dimension!(props::Left);
impl_valid_for_dimension!(props::Right);
impl_valid_for_dimension!(props::Bottom);
impl_valid_for_dimension!(props::Outline);

impl ValidFor<props::ZIndex> for i32 {}
impl ValidFor<props::ZIndex> for u32 {}
impl ValidFor<props::ZIndex> for i64 {}
impl ValidFor<props::ZIndex> for u64 {}
impl ValidFor<props::ZIndex> for isize {}
impl ValidFor<props::ZIndex> for usize {}

impl ValidFor<props::FlexGrow> for i32 {}
impl ValidFor<props::FlexGrow> for f64 {}
impl ValidFor<props::FlexShrink> for i32 {}
impl ValidFor<props::FlexShrink> for f64 {}
impl ValidFor<props::Opacity> for f64 {}
impl ValidFor<props::Opacity> for f32 {}
impl ValidFor<props::FontWeight> for i32 {}
impl ValidFor<props::FontWeight> for u32 {}

impl ValidFor<props::Color> for Rgba {}

impl ValidFor<props::BackgroundColor> for Rgba {}
impl ValidFor<props::BorderColor> for Rgba {}
impl ValidFor<props::Background> for Rgba {}
impl ValidFor<props::Outline> for Rgba {}

#[derive(Clone, Copy, Debug)]
pub enum DisplayKeyword {
    Flex,
    Block,
    Grid,
    Inline,
    InlineBlock,
    InlineFlex,
    None,
}

impl Display for DisplayKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Flex => write!(f, "flex"),
            Self::Block => write!(f, "block"),
            Self::Grid => write!(f, "grid"),
            Self::Inline => write!(f, "inline"),
            Self::InlineBlock => write!(f, "inline-block"),
            Self::InlineFlex => write!(f, "inline-flex"),
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

#[derive(Clone, Copy, Debug)]
pub enum CursorKeyword {
    Auto,
    Default,
    Pointer,
    Wait,
    Text,
    Move,
    Help,
    NotAllowed,
}

impl Display for CursorKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Default => write!(f, "default"),
            Self::Pointer => write!(f, "pointer"),
            Self::Wait => write!(f, "wait"),
            Self::Text => write!(f, "text"),
            Self::Move => write!(f, "move"),
            Self::Help => write!(f, "help"),
            Self::NotAllowed => write!(f, "not-allowed"),
        }
    }
}
impl ValidFor<props::Cursor> for CursorKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignItemsKeyword {
    Stretch,
    Center,
    FlexStart,
    FlexEnd,
    Baseline,
}

impl Display for AlignItemsKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stretch => write!(f, "stretch"),
            Self::Center => write!(f, "center"),
            Self::FlexStart => write!(f, "flex-start"),
            Self::FlexEnd => write!(f, "flex-end"),
            Self::Baseline => write!(f, "baseline"),
        }
    }
}
impl ValidFor<props::AlignItems> for AlignItemsKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JustifyContentKeyword {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

impl Display for JustifyContentKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FlexStart => write!(f, "flex-start"),
            Self::FlexEnd => write!(f, "flex-end"),
            Self::Center => write!(f, "center"),
            Self::SpaceBetween => write!(f, "space-between"),
            Self::SpaceAround => write!(f, "space-around"),
            Self::SpaceEvenly => write!(f, "space-evenly"),
        }
    }
}
impl ValidFor<props::JustifyContent> for JustifyContentKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlexWrapKeyword {
    Nowrap,
    Wrap,
    WrapReverse,
}

impl Display for FlexWrapKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nowrap => write!(f, "nowrap"),
            Self::Wrap => write!(f, "wrap"),
            Self::WrapReverse => write!(f, "wrap-reverse"),
        }
    }
}
impl ValidFor<props::FlexWrap> for FlexWrapKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VisibilityKeyword {
    Visible,
    Hidden,
    Collapse,
}

impl Display for VisibilityKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Visible => write!(f, "visible"),
            Self::Hidden => write!(f, "hidden"),
            Self::Collapse => write!(f, "collapse"),
        }
    }
}
impl ValidFor<props::Visibility> for VisibilityKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverflowKeyword {
    Visible,
    Hidden,
    Scroll,
    Auto,
}

impl Display for OverflowKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Visible => write!(f, "visible"),
            Self::Hidden => write!(f, "hidden"),
            Self::Scroll => write!(f, "scroll"),
            Self::Auto => write!(f, "auto"),
        }
    }
}
impl ValidFor<props::Overflow> for OverflowKeyword {}
impl ValidFor<props::OverflowX> for OverflowKeyword {}
impl ValidFor<props::OverflowY> for OverflowKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlignKeyword {
    Left,
    Right,
    Center,
    Justify,
}

impl Display for TextAlignKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left => write!(f, "left"),
            Self::Right => write!(f, "right"),
            Self::Center => write!(f, "center"),
            Self::Justify => write!(f, "justify"),
        }
    }
}
impl ValidFor<props::TextAlign> for TextAlignKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontWeightKeyword {
    Normal,
    Bold,
    Bolder,
    Lighter,
    W100,
    W200,
    W300,
    W400,
    W500,
    W600,
    W700,
    W800,
    W900,
}

impl Display for FontWeightKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Bold => write!(f, "bold"),
            Self::Bolder => write!(f, "bolder"),
            Self::Lighter => write!(f, "lighter"),
            Self::W100 => write!(f, "100"),
            Self::W200 => write!(f, "200"),
            Self::W300 => write!(f, "300"),
            Self::W400 => write!(f, "400"),
            Self::W500 => write!(f, "500"),
            Self::W600 => write!(f, "600"),
            Self::W700 => write!(f, "700"),
            Self::W800 => write!(f, "800"),
            Self::W900 => write!(f, "900"),
        }
    }
}
impl ValidFor<props::FontWeight> for FontWeightKeyword {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEventsKeyword {
    Auto,
    None,
}

impl Display for PointerEventsKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::None => write!(f, "none"),
        }
    }
}
impl ValidFor<props::PointerEvents> for PointerEventsKeyword {}

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
impl ValidFor<props::Background> for Hex {}
impl ValidFor<props::Outline> for Hex {}

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
impl ValidFor<props::Background> for Hsl {}
impl ValidFor<props::Outline> for Hsl {}

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
impl ValidFor<props::Background> for Url {}

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
    pub fn top<V: ValidFor<props::Top> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn right<V: ValidFor<props::Right> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn bottom<V: ValidFor<props::Bottom> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn left<V: ValidFor<props::Left> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
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
    pub fn top<V: ValidFor<props::Top> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn right<V: ValidFor<props::Right> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn bottom<V: ValidFor<props::Bottom> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
    }
    pub fn left<V: ValidFor<props::Left> + Display>(v: V) -> UnsafeCss {
        UnsafeCss::new(format!("{}", v))
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
impl ValidFor<props::BorderRadius> for UnsafeCss {}
impl ValidFor<props::FontSize> for UnsafeCss {}
impl ValidFor<props::FontWeight> for UnsafeCss {}
impl ValidFor<props::LetterSpacing> for UnsafeCss {}
impl ValidFor<props::LineHeight> for UnsafeCss {}
impl ValidFor<props::TextAlign> for UnsafeCss {}
impl ValidFor<props::TextDecoration> for UnsafeCss {}
impl ValidFor<props::Cursor> for UnsafeCss {}
impl ValidFor<props::Gap> for UnsafeCss {}
impl ValidFor<props::AlignItems> for UnsafeCss {}
impl ValidFor<props::JustifyContent> for UnsafeCss {}
impl ValidFor<props::FlexWrap> for UnsafeCss {}
impl ValidFor<props::FlexGrow> for UnsafeCss {}
impl ValidFor<props::FlexShrink> for UnsafeCss {}
impl ValidFor<props::FlexBasis> for UnsafeCss {}
impl ValidFor<props::Top> for UnsafeCss {}
impl ValidFor<props::Left> for UnsafeCss {}
impl ValidFor<props::Right> for UnsafeCss {}
impl ValidFor<props::Bottom> for UnsafeCss {}
impl ValidFor<props::Opacity> for UnsafeCss {}
impl ValidFor<props::Visibility> for UnsafeCss {}
impl ValidFor<props::PointerEvents> for UnsafeCss {}
impl ValidFor<props::Overflow> for UnsafeCss {}
impl ValidFor<props::OverflowX> for UnsafeCss {}
impl ValidFor<props::OverflowY> for UnsafeCss {}
impl ValidFor<props::Transition> for UnsafeCss {}
impl ValidFor<props::Transform> for UnsafeCss {}
impl ValidFor<props::BoxShadow> for UnsafeCss {}
impl ValidFor<props::BackdropFilter> for UnsafeCss {}
impl ValidFor<props::Filter> for UnsafeCss {}
impl ValidFor<props::Background> for UnsafeCss {}
impl ValidFor<props::Outline> for UnsafeCss {}

// Helper for String values to support literals in builder
impl ValidFor<props::Transition> for String {}
impl ValidFor<props::Transform> for String {}
impl ValidFor<props::BoxShadow> for String {}
impl ValidFor<props::BackdropFilter> for String {}
impl ValidFor<props::Filter> for String {}
impl ValidFor<props::Background> for String {}
impl ValidFor<props::TextDecoration> for String {}

impl ValidFor<props::Transition> for &'static str {}
impl ValidFor<props::Transform> for &'static str {}
impl ValidFor<props::BoxShadow> for &'static str {}
impl ValidFor<props::BackdropFilter> for &'static str {}
impl ValidFor<props::Filter> for &'static str {}
impl ValidFor<props::Background> for &'static str {}
impl ValidFor<props::TextDecoration> for &'static str {}

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

                fn is_constant_value(&self) -> bool {
                    true
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
    UnsafeCss,
    CursorKeyword,
    AlignItemsKeyword,
    JustifyContentKeyword,
    FlexWrapKeyword,
    VisibilityKeyword,
    OverflowKeyword,
    TextAlignKeyword,
    FontWeightKeyword,
    PointerEventsKeyword
);
