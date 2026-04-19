use std::fmt::Display;

// ==========================================
// 核心包裹单元类型 (Units)
// ==========================================

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Px(pub Option<f64>);
impl Display for Px {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}px", v)
        } else {
            Ok(())
        }
    }
}

impl From<i32> for Px {
    fn from(v: i32) -> Self {
        Px(Some(v as f64))
    }
}

impl From<f64> for Px {
    fn from(v: f64) -> Self {
        Px(Some(v))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Percent(pub Option<f64>);
impl Display for Percent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}%", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba(pub Option<(u8, u8, u8, f64)>);
impl Rgba {
    pub fn alpha(self, alpha: f64) -> Self {
        if let Some((r, g, b, _)) = self.0 {
            Self(Some((r, g, b, alpha)))
        } else {
            self
        }
    }
}
impl Display for Rgba {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some((r, g, b, a)) = self.0 {
            write!(f, "rgba({}, {}, {}, {})", r, g, b, a)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Auto(pub Option<()>);
impl Display for Auto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "none")
    }
}

pub const NONE: NoneValue = NoneValue;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rem(pub Option<f64>);
impl Display for Rem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}rem", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Em(pub Option<f64>);
impl Display for Em {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}em", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vw(pub Option<f64>);
impl Display for Vw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}vw", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vh(pub Option<f64>);
impl Display for Vh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}vh", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Deg(pub Option<f64>);
impl Display for Deg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}deg", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rad(pub Option<f64>);
impl Display for Rad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}rad", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Turn(pub Option<f64>);
impl Display for Turn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = self.0 {
            write!(f, "{}turn", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hex(pub String);
impl Hex {
    pub fn alpha(self, alpha: f64) -> Hex {
        let alpha_hex = (alpha * 255.0) as u8;
        Hex(format!("{}{:02x}", self.0, alpha_hex))
    }
}
impl Display for Hex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsl(pub Option<(u16, u8, u8, f64)>);
impl Hsl {
    pub fn alpha(self, alpha: f64) -> Self {
        if let Some((h, s, l, _)) = self.0 {
            Self(Some((h, s, l, alpha)))
        } else {
            self
        }
    }
}
impl Display for Hsl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some((h, s, l, a)) = self.0 {
            if (a - 1.0).abs() < f64::EPSILON {
                write!(f, "hsl({}, {}%, {}%)", h, s, l)
            } else {
                write!(f, "hsla({}, {}%, {}%, {})", h, s, l, a)
            }
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Url(pub String);
impl Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "url('{}')", self.0)
    }
}

#[inline]
pub fn px<T: Into<f64>>(v: T) -> Px {
    Px(Some(v.into()))
}
#[inline]
pub fn pct<T: Into<f64>>(v: T) -> Percent {
    Percent(Some(v.into()))
}
#[inline]
pub fn rem<T: Into<f64>>(v: T) -> Rem {
    Rem(Some(v.into()))
}
#[inline]
pub fn em_unit<T: Into<f64>>(v: T) -> Em {
    Em(Some(v.into()))
}
#[inline]
pub fn vw<T: Into<f64>>(v: T) -> Vw {
    Vw(Some(v.into()))
}
#[inline]
pub fn vh<T: Into<f64>>(v: T) -> Vh {
    Vh(Some(v.into()))
}
#[inline]
pub fn deg<T: Into<f64>>(v: T) -> Deg {
    Deg(Some(v.into()))
}
#[inline]
pub fn rad<T: Into<f64>>(v: T) -> Rad {
    Rad(Some(v.into()))
}
#[inline]
pub fn turn<T: Into<f64>>(v: T) -> Turn {
    Turn(Some(v.into()))
}
#[inline]
pub fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba(Some((r, g, b, 1.0)))
}
#[inline]
pub fn rgba<T: Into<f64>>(r: u8, g: u8, b: u8, a: T) -> Rgba {
    Rgba(Some((r, g, b, a.into())))
}
#[inline]
pub fn hex<T: Into<String>>(v: T) -> Hex {
    Hex(v.into())
}
#[inline]
pub fn hsl(h: u16, s: u8, l: u8) -> Hsl {
    Hsl(Some((h, s, l, 1.0)))
}
#[inline]
pub fn hsla<A: Into<f64>>(h: u16, s: u8, l: u8, a: A) -> Hsl {
    Hsl(Some((h, s, l, a.into())))
}
#[inline]
pub fn url<T: Into<String>>(v: T) -> Url {
    Url(v.into())
}
