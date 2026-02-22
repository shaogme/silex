use std::fmt::Display;

// ==========================================
// 核心包裹单元类型 (Units)
// ==========================================

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percent(pub f64);
impl Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub f32);
impl Display for Rgba {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rgba({}, {}, {}, {})", self.0, self.1, self.2, self.3)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Auto;
impl Display for Auto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "auto")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rem(pub f64);
impl Display for Rem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}rem", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Em(pub f64);
impl Display for Em {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}em", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vw(pub f64);
impl Display for Vw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}vw", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vh(pub f64);
impl Display for Vh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}vh", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deg(pub f64);
impl Display for Deg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}deg", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rad(pub f64);
impl Display for Rad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}rad", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn(pub f64);
impl Display for Turn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}turn", self.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsl(pub u16, pub u8, pub u8);
impl Display for Hsl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hsl({}, {}%, {}%)", self.0, self.1, self.2)
    }
}

#[derive(Clone, Debug, PartialEq)]
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
