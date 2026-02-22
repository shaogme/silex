use std::fmt::Display;
use std::marker::PhantomData;

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

// ==========================================
// 核心标记 Trait 与 量纲 (Marks & Marker Traits)
// ==========================================

#[derive(Clone, Copy, Debug)]
pub struct LengthMark;
#[derive(Clone, Copy, Debug)]
pub struct AngleMark;
#[derive(Clone, Copy, Debug)]
pub struct NumberMark;
#[derive(Clone, Copy, Debug)]
pub struct ColorMark;

pub trait CssLength: Display {}
pub trait CssAngle: Display {}
pub trait CssColor: Display {}
pub trait CssNumber: Display {}
pub trait CssPercentage: Display {}

#[derive(Clone, Debug)]
pub struct CalcValue<Mark>(pub String, pub PhantomData<Mark>);

impl<Mark> CalcValue<Mark> {
    pub fn new(s: String) -> Self {
        Self(s, PhantomData)
    }
    pub fn binary<L: Display, R: Display>(l: L, op: &'static str, r: R) -> Self {
        Self(format!("({} {} {})", l, op, r), PhantomData)
    }
}

impl<Mark> Display for CalcValue<Mark> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl CssLength for CalcValue<LengthMark> {}
impl CssAngle for CalcValue<AngleMark> {}

impl<Mark> From<CalcValue<Mark>> for String {
    fn from(v: CalcValue<Mark>) -> Self {
        v.0
    }
}

pub fn calc<Mark>(v: CalcValue<Mark>) -> CalcValue<Mark> {
    CalcValue::new(format!("calc({})", v.0))
}

pub fn min<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    let mut s = String::from("min(");
    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        let v: CalcValue<Mark> = arg.into();
        s.push_str(&v.0);
    }
    s.push(')');
    CalcValue::new(s)
}

pub fn max<Mark, T, I>(args: I) -> CalcValue<Mark>
where
    I: IntoIterator<Item = T>,
    T: Into<CalcValue<Mark>>,
{
    let mut s = String::from("max(");
    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        let v: CalcValue<Mark> = arg.into();
        s.push_str(&v.0);
    }
    s.push(')');
    CalcValue::new(s)
}

pub fn clamp<Mark, T>(min_v: T, val: T, max_v: T) -> CalcValue<Mark>
where
    T: Into<CalcValue<Mark>> + Display,
{
    CalcValue::new(format!("clamp({}, {}, {})", min_v, val, max_v))
}

// ==========================================
// 核心包裹单元类型 (Units)
// ==========================================

#[derive(Clone, Copy, Debug, Default)]
pub struct Px(pub f64);
impl Display for Px {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}px", self.0)
    }
}

impl From<i32> for Px {
    fn from(v: i32) -> Self {
        Px(v as f64)
    }
}

impl From<f64> for Px {
    fn from(v: f64) -> Self {
        Px(v)
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

#[derive(Clone, Copy, Debug)]
pub struct Rem(pub f64);
impl Display for Rem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}rem", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Em(pub f64);
impl Display for Em {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}em", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vw(pub f64);
impl Display for Vw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}vw", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vh(pub f64);
impl Display for Vh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}vh", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Deg(pub f64);
impl Display for Deg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}deg", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rad(pub f64);
impl Display for Rad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}rad", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Turn(pub f64);
impl Display for Turn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}turn", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Hex(pub String);

impl Default for Hex {
    fn default() -> Self {
        Self("#000000".to_string())
    }
}
impl Display for Hex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Hsl(pub u16, pub u8, pub u8);
impl Display for Hsl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hsl({}, {}%, {}%)", self.0, self.1, self.2)
    }
}

#[derive(Clone, Debug)]
pub struct Url(pub String);
impl Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "url('{}')", self.0)
    }
}

#[inline]
pub fn px<T: Into<f64>>(v: T) -> Px {
    Px(v.into())
}
#[inline]
pub fn pct<T: Into<f64>>(v: T) -> Percent {
    Percent(v.into())
}
#[inline]
pub fn rem<T: Into<f64>>(v: T) -> Rem {
    Rem(v.into())
}
#[inline]
pub fn em<T: Into<f64>>(v: T) -> Em {
    Em(v.into())
}
#[inline]
pub fn vw<T: Into<f64>>(v: T) -> Vw {
    Vw(v.into())
}
#[inline]
pub fn vh<T: Into<f64>>(v: T) -> Vh {
    Vh(v.into())
}
#[inline]
pub fn deg<T: Into<f64>>(v: T) -> Deg {
    Deg(v.into())
}
#[inline]
pub fn rad<T: Into<f64>>(v: T) -> Rad {
    Rad(v.into())
}
#[inline]
pub fn turn<T: Into<f64>>(v: T) -> Turn {
    Turn(v.into())
}
#[inline]
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    Rgba(r, g, b, a)
}
#[inline]
pub fn hex<T: Into<String>>(v: T) -> Hex {
    Hex(v.into())
}
#[inline]
pub fn hsl(h: u16, s: u8, l: u8) -> Hsl {
    Hsl(h, s, l)
}
#[inline]
pub fn url<T: Into<String>>(v: T) -> Url {
    Url(v.into())
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
        $(impl ValidFor<$prop> for $name {})*
    };
}

include!("keywords_gen.rs");

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

pub fn border<W, S, C>(width: W, style: S, color: C) -> BorderValue
where
    W: ValidFor<props::BorderWidth> + Display,
    S: ValidFor<props::BorderStyle> + Display,
    C: ValidFor<props::BorderColor> + Display,
{
    BorderValue(format!("{} {} {}", width, style, color))
}

#[derive(Clone, Debug)]
pub struct MarginValue(pub String);
impl Display for MarginValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ValidFor<props::Margin> for MarginValue {}

pub mod margin {
    use super::*;
    pub fn all<T: ValidFor<props::Margin> + Display>(v: T) -> MarginValue {
        MarginValue(format!("{}", v))
    }
    pub fn x_y<X, Y>(x: X, y: Y) -> MarginValue
    where
        X: ValidFor<props::Margin> + Display,
        Y: ValidFor<props::Margin> + Display,
    {
        MarginValue(format!("{} {}", x, y))
    }
    pub fn top_right_bottom_left<T, R, B, L>(top: T, right: R, bottom: B, left: L) -> MarginValue
    where
        T: ValidFor<props::Margin> + Display,
        R: ValidFor<props::Margin> + Display,
        B: ValidFor<props::Margin> + Display,
        L: ValidFor<props::Margin> + Display,
    {
        MarginValue(format!("{} {} {} {}", top, right, bottom, left))
    }
    pub fn top<V: ValidFor<props::Top> + Display>(v: V) -> MarginValue {
        MarginValue(format!("{}", v))
    }
    pub fn right<V: ValidFor<props::Right> + Display>(v: V) -> MarginValue {
        MarginValue(format!("{}", v))
    }
    pub fn bottom<V: ValidFor<props::Bottom> + Display>(v: V) -> MarginValue {
        MarginValue(format!("{}", v))
    }
    pub fn left<V: ValidFor<props::Left> + Display>(v: V) -> MarginValue {
        MarginValue(format!("{}", v))
    }
}

#[derive(Clone, Debug)]
pub struct PaddingValue(pub String);
impl Display for PaddingValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ValidFor<props::Padding> for PaddingValue {}

pub mod padding {
    use super::*;
    pub fn all<T: ValidFor<props::Padding> + Display>(v: T) -> PaddingValue {
        PaddingValue(format!("{}", v))
    }
    pub fn x_y<X, Y>(x: X, y: Y) -> PaddingValue
    where
        X: ValidFor<props::Padding> + Display,
        Y: ValidFor<props::Padding> + Display,
    {
        PaddingValue(format!("{} {}", x, y))
    }
    pub fn top_right_bottom_left<T, R, B, L>(top: T, right: R, bottom: B, left: L) -> PaddingValue
    where
        T: ValidFor<props::Padding> + Display,
        R: ValidFor<props::Padding> + Display,
        B: ValidFor<props::Padding> + Display,
        L: ValidFor<props::Padding> + Display,
    {
        PaddingValue(format!("{} {} {} {}", top, right, bottom, left))
    }
    pub fn top<V: ValidFor<props::Top> + Display>(v: V) -> PaddingValue {
        PaddingValue(format!("{}", v))
    }
    pub fn right<V: ValidFor<props::Right> + Display>(v: V) -> PaddingValue {
        PaddingValue(format!("{}", v))
    }
    pub fn bottom<V: ValidFor<props::Bottom> + Display>(v: V) -> PaddingValue {
        PaddingValue(format!("{}", v))
    }
    pub fn left<V: ValidFor<props::Left> + Display>(v: V) -> PaddingValue {
        PaddingValue(format!("{}", v))
    }
}

#[derive(Clone, Debug)]
pub struct FlexValue(pub String);
impl Display for FlexValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ValidFor<props::Flex> for FlexValue {}

pub fn flex<G, S, B>(grow: G, shrink: S, basis: B) -> FlexValue
where
    G: ValidFor<props::FlexGrow> + Display,
    S: ValidFor<props::FlexShrink> + Display,
    B: ValidFor<props::FlexBasis> + Display,
{
    FlexValue(format!("{} {} {}", grow, shrink, basis))
}

#[derive(Clone, Debug)]
pub struct TransitionValue(pub String);
impl Display for TransitionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ValidFor<props::Transition> for TransitionValue {}

pub fn transition<P, D, T, E>(property: P, duration: D, timing: T, delay: E) -> TransitionValue
where
    P: ValidFor<props::TransitionProperty> + Display,
    D: ValidFor<props::TransitionDuration> + Display,
    T: ValidFor<props::TransitionTimingFunction> + Display,
    E: ValidFor<props::TransitionDelay> + Display,
{
    TransitionValue(format!("{} {} {} {}", property, duration, timing, delay))
}

#[derive(Clone, Debug)]
pub struct BackgroundValue(pub String);
impl Display for BackgroundValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ValidFor<props::Background> for BackgroundValue {}

pub fn background<C, I>(color: C, image: I) -> BackgroundValue
where
    C: ValidFor<props::BackgroundColor> + Display,
    I: ValidFor<props::BackgroundImage> + Display,
{
    BackgroundValue(format!("{} {}", color, image))
}

// ==========================================
// 渐变 DSL (Gradients)
// ==========================================

#[derive(Clone, Debug)]
pub struct GradientValue(pub String);
impl Display for GradientValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ValidFor<props::BackgroundImage> for GradientValue {}
impl ValidFor<props::Background> for GradientValue {}

#[derive(Clone, Debug)]
pub enum Direction {
    ToTop,
    ToBottom,
    ToLeft,
    ToRight,
    ToTopLeft,
    ToTopRight,
    ToBottomLeft,
    ToBottomRight,
    Angle(CalcValue<AngleMark>),
}

impl Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ToTop => write!(f, "to top"),
            Self::ToBottom => write!(f, "to bottom"),
            Self::ToLeft => write!(f, "to left"),
            Self::ToRight => write!(f, "to right"),
            Self::ToTopLeft => write!(f, "to top left"),
            Self::ToTopRight => write!(f, "to top right"),
            Self::ToBottomLeft => write!(f, "to bottom left"),
            Self::ToBottomRight => write!(f, "to bottom right"),
            Self::Angle(a) => write!(f, "{}", a),
        }
    }
}

impl From<Deg> for Direction {
    fn from(v: Deg) -> Self {
        Self::Angle(v.into())
    }
}

#[derive(Clone, Debug)]
pub struct ColorStop {
    pub color: String,
    pub position: Option<String>,
}

impl Display for ColorStop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.color)?;
        if let Some(pos) = &self.position {
            write!(f, " {}", pos)?;
        }
        Ok(())
    }
}

pub struct LinearGradientBuilder {
    direction: Option<Direction>,
    stops: Vec<ColorStop>,
    repeating: bool,
}

impl Default for LinearGradientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearGradientBuilder {
    pub fn new() -> Self {
        Self {
            direction: None,
            stops: Vec::new(),
            repeating: false,
        }
    }
    pub fn to(mut self, dir: Direction) -> Self {
        self.direction = Some(dir);
        self
    }
    pub fn stop<C: CssColor, P: Display>(mut self, color: C, pos: impl Into<Option<P>>) -> Self {
        self.stops.push(ColorStop {
            color: color.to_string(),
            position: pos.into().map(|p| p.to_string()),
        });
        self
    }
    pub fn repeating(mut self) -> Self {
        self.repeating = true;
        self
    }
    pub fn build(self) -> GradientValue {
        let name = if self.repeating {
            "repeating-linear-gradient"
        } else {
            "linear-gradient"
        };
        let mut s = format!("{}(", name);
        let mut first = true;
        if let Some(dir) = self.direction {
            s.push_str(&dir.to_string());
            first = false;
        }
        for stop in self.stops {
            if !first {
                s.push_str(", ");
            }
            s.push_str(&stop.to_string());
            first = false;
        }
        s.push(')');
        GradientValue(s)
    }
}

pub fn linear_gradient() -> LinearGradientBuilder {
    LinearGradientBuilder::new()
}

pub struct RadialGradientBuilder {
    shape_size: Option<String>,
    position: Option<String>,
    stops: Vec<ColorStop>,
    repeating: bool,
}

impl Default for RadialGradientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RadialGradientBuilder {
    pub fn new() -> Self {
        Self {
            shape_size: None,
            position: None,
            stops: Vec::new(),
            repeating: false,
        }
    }
    pub fn circle(mut self) -> Self {
        self.shape_size = Some("circle".to_string());
        self
    }
    pub fn ellipse(mut self) -> Self {
        self.shape_size = Some("ellipse".to_string());
        self
    }
    pub fn at<P: Display>(mut self, pos: P) -> Self {
        self.position = Some(pos.to_string());
        self
    }
    pub fn stop<C: CssColor, P: Display>(mut self, color: C, pos: impl Into<Option<P>>) -> Self {
        self.stops.push(ColorStop {
            color: color.to_string(),
            position: pos.into().map(|p| p.to_string()),
        });
        self
    }
    pub fn repeating(mut self) -> Self {
        self.repeating = true;
        self
    }
    pub fn build(self) -> GradientValue {
        let name = if self.repeating {
            "repeating-radial-gradient"
        } else {
            "radial-gradient"
        };
        let mut s = format!("{}(", name);
        let mut first = true;

        if let Some(ss) = &self.shape_size {
            s.push_str(ss);
            first = false;
        }

        if let Some(pos) = &self.position {
            if !first {
                s.push(' ');
            }
            s.push_str("at ");
            s.push_str(pos);
            first = false;
        }

        for stop in self.stops {
            if !first {
                s.push_str(", ");
            }
            s.push_str(&stop.to_string());
            first = false;
        }
        s.push(')');
        GradientValue(s)
    }
}

pub fn radial_gradient() -> RadialGradientBuilder {
    RadialGradientBuilder::new()
}

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

// ==========================================
// 属性定义与基础约束自动化 (放在最后以确保类型已定义)
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

trait IntoCalc<Mark> {
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
}

// 调用中心注册表执行代码生成
crate::for_all_properties!(define_props);

// --- 手动补充跨组约束 ---
impl ValidFor<props::Border> for BorderValue {}
impl ValidFor<props::Background> for Url {}
impl ValidFor<props::BackgroundImage> for Url {}
impl<T: Display> ValidFor<props::Any> for T {}

// ==========================================
// 响应式信号集成 (Reactivity Integration)
// ==========================================

macro_rules! impl_into_signal_for_css {
    ($($t:ty),*) => {
        $(
            impl silex_core::traits::IntoSignal for $t {
                type Value = $t;
                type Signal = silex_core::reactivity::Constant<$t>;
                fn into_signal(self) -> Self::Signal { silex_core::reactivity::Constant(self) }
                fn is_constant_value(&self) -> bool { true }
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
    CalcValue<LengthMark>,
    CalcValue<AngleMark>,
    Deg,
    Rad,
    Turn,
    GradientValue
);

register_generated_keywords!(impl_into_signal_for_css);
