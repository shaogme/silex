use std::fmt::{Display, Formatter, Result};

// ==========================================
// 核心包裹单元类型 (Units)
// ==========================================

macro_rules! impl_string_value_wrapper {
    ($t:ident) => {
        impl $t {
            #[inline]
            pub fn as_str(&self) -> &str {
                &self.0
            }
            #[inline]
            pub fn into_inner(self) -> String {
                self.0
            }
            #[inline]
            pub fn set_value(&mut self, val: impl Into<String>) {
                self.0 = val.into();
            }
            #[inline]
            pub fn with_value(mut self, val: impl Into<String>) -> Self {
                self.0 = val.into();
                self
            }
        }
        impl ::std::fmt::Display for $t {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}
pub(crate) use impl_string_value_wrapper;

macro_rules! impl_unit_value_methods {
    ($t:ty) => {
        impl $t {
            #[inline]
            pub const fn value(&self) -> f64 {
                self.0
            }
            #[inline]
            pub fn set_value(&mut self, val: impl Into<f64>) {
                self.0 = val.into();
            }
            #[inline]
            pub fn with_value(mut self, val: impl Into<f64>) -> Self {
                self.0 = val.into();
                self
            }
            #[inline]
            pub fn map(mut self, f: impl FnOnce(f64) -> f64) -> Self {
                self.0 = f(self.0);
                self
            }
            #[inline]
            pub fn update(&mut self, f: impl FnOnce(f64) -> f64) {
                self.0 = f(self.0);
            }
        }
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Px(f64);
impl_unit_value_methods!(Px);
impl Display for Px {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Percent(f64);
impl_unit_value_methods!(Percent);
impl Display for Percent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}%", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba(u8, u8, u8, f64);
impl Rgba {
    #[inline]
    pub const fn red(&self) -> u8 {
        self.0
    }
    #[inline]
    pub const fn green(&self) -> u8 {
        self.1
    }
    #[inline]
    pub const fn blue(&self) -> u8 {
        self.2
    }
    #[inline]
    pub const fn alpha_val(&self) -> f64 {
        self.3
    }
    #[inline]
    pub const fn channels(&self) -> (u8, u8, u8, f64) {
        (self.0, self.1, self.2, self.3)
    }
    #[inline]
    pub fn set_red(&mut self, r: u8) {
        self.0 = r;
    }
    #[inline]
    pub fn set_green(&mut self, g: u8) {
        self.1 = g;
    }
    #[inline]
    pub fn set_blue(&mut self, b: u8) {
        self.2 = b;
    }
    #[inline]
    pub fn set_alpha(&mut self, a: impl Into<f64>) {
        self.3 = a.into();
    }
    #[inline]
    pub fn with_red(mut self, r: u8) -> Self {
        self.0 = r;
        self
    }
    #[inline]
    pub fn with_green(mut self, g: u8) -> Self {
        self.1 = g;
        self
    }
    #[inline]
    pub fn with_blue(mut self, b: u8) -> Self {
        self.2 = b;
        self
    }
    #[inline]
    pub fn with_alpha(mut self, a: impl Into<f64>) -> Self {
        self.3 = a.into();
        self
    }
    #[inline]
    pub fn alpha(mut self, alpha: impl Into<f64>) -> Self {
        self.3 = alpha.into();
        self
    }
}
impl Display for Rgba {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let Rgba(r, g, b, a) = *self;
        write!(f, "rgba({}, {}, {}, {})", r, g, b, a)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Auto(Option<()>);
impl Display for Auto {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.0.is_some() {
            write!(f, "auto")
        } else {
            Ok(())
        }
    }
}

pub const AUTO: Auto = Auto(Some(()));

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NoneValue;
impl Display for NoneValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "none")
    }
}

pub const NONE: NoneValue = NoneValue;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rem(f64);
impl_unit_value_methods!(Rem);
impl Display for Rem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}rem", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Em(f64);
impl_unit_value_methods!(Em);
impl Display for Em {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}em", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vw(f64);
impl_unit_value_methods!(Vw);
impl Display for Vw {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}vw", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vh(f64);
impl_unit_value_methods!(Vh);
impl Display for Vh {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}vh", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Deg(f64);
impl_unit_value_methods!(Deg);
impl Display for Deg {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}deg", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rad(f64);
impl_unit_value_methods!(Rad);
impl Display for Rad {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}rad", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn(f64);
impl_unit_value_methods!(Turn);
impl Display for Turn {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}turn", self.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hex(String);
impl_string_value_wrapper!(Hex);
impl Hex {
    pub fn alpha(self, alpha: f64) -> Hex {
        let alpha_hex = (alpha * 255.0) as u8;
        Hex(format!("{}{:02x}", self.0, alpha_hex))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsl(u16, u8, u8, f64);
impl Hsl {
    #[inline]
    pub const fn hue(&self) -> u16 {
        self.0
    }
    #[inline]
    pub const fn saturation(&self) -> u8 {
        self.1
    }
    #[inline]
    pub const fn lightness(&self) -> u8 {
        self.2
    }
    #[inline]
    pub const fn alpha_val(&self) -> f64 {
        self.3
    }
    #[inline]
    pub const fn channels(&self) -> (u16, u8, u8, f64) {
        (self.0, self.1, self.2, self.3)
    }
    #[inline]
    pub fn set_hue(&mut self, h: u16) {
        self.0 = h;
    }
    #[inline]
    pub fn set_saturation(&mut self, s: u8) {
        self.1 = s;
    }
    #[inline]
    pub fn set_lightness(&mut self, l: u8) {
        self.2 = l;
    }
    #[inline]
    pub fn set_alpha(&mut self, a: impl Into<f64>) {
        self.3 = a.into();
    }
    #[inline]
    pub fn with_hue(mut self, h: u16) -> Self {
        self.0 = h;
        self
    }
    #[inline]
    pub fn with_saturation(mut self, s: u8) -> Self {
        self.1 = s;
        self
    }
    #[inline]
    pub fn with_lightness(mut self, l: u8) -> Self {
        self.2 = l;
        self
    }
    #[inline]
    pub fn with_alpha(mut self, a: impl Into<f64>) -> Self {
        self.3 = a.into();
        self
    }
    #[inline]
    pub fn alpha(mut self, alpha: impl Into<f64>) -> Self {
        self.3 = alpha.into();
        self
    }
}
impl Display for Hsl {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let (h, s, l, a) = self.channels();
        if (a - 1.0).abs() < f64::EPSILON {
            write!(f, "hsl({}, {}%, {}%)", h, s, l)
        } else {
            write!(f, "hsla({}, {}%, {}%, {})", h, s, l, a)
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Url(String);
impl_string_value_wrapper!(Url);

#[inline(always)]
pub fn px<T: Into<f64>>(v: T) -> Px {
    Px(v.into())
}
#[inline(always)]
pub fn pct<T: Into<f64>>(v: T) -> Percent {
    Percent(v.into())
}
#[inline(always)]
pub fn rem<T: Into<f64>>(v: T) -> Rem {
    Rem(v.into())
}
#[inline(always)]
pub fn em_unit<T: Into<f64>>(v: T) -> Em {
    Em(v.into())
}
#[inline(always)]
pub fn vw<T: Into<f64>>(v: T) -> Vw {
    Vw(v.into())
}
#[inline(always)]
pub fn vh<T: Into<f64>>(v: T) -> Vh {
    Vh(v.into())
}
#[inline(always)]
pub fn deg<T: Into<f64>>(v: T) -> Deg {
    Deg(v.into())
}
#[inline(always)]
pub fn rad<T: Into<f64>>(v: T) -> Rad {
    Rad(v.into())
}
#[inline(always)]
pub fn turn<T: Into<f64>>(v: T) -> Turn {
    Turn(v.into())
}
#[inline(always)]
pub fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba(r, g, b, 1.0)
}
#[inline(always)]
pub fn rgba<T: Into<f64>>(r: u8, g: u8, b: u8, a: T) -> Rgba {
    Rgba(r, g, b, a.into())
}
#[inline(always)]
pub fn hex<T: Into<String>>(v: T) -> Hex {
    Hex(v.into())
}
#[inline(always)]
pub fn hsl(h: u16, s: u8, l: u8) -> Hsl {
    Hsl(h, s, l, 1.0)
}
#[inline(always)]
pub fn hsla<A: Into<f64>>(h: u16, s: u8, l: u8, a: A) -> Hsl {
    Hsl(h, s, l, a.into())
}
#[inline(always)]
pub fn url<T: Into<String>>(v: T) -> Url {
    Url(v.into())
}
