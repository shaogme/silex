use std::fmt::Display;

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

#[derive(Clone, Debug)]
pub struct Hex(pub String);
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
        impl ValidFor<$prop> for Auto {}
    };
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
    UnsafeCss
);

register_generated_keywords!(impl_into_signal_for_css);
