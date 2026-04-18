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
pub struct Rgba(pub Option<(u8, u8, u8, f32)>);
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
pub struct Hex(pub Option<String>);
impl Display for Hex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = &self.0 {
            write!(f, "{}", v)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hsl(pub Option<(u16, u8, u8)>);
impl Display for Hsl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some((h, s, l)) = self.0 {
            write!(f, "hsl({}, {}%, {}%)", h, s, l)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Url(pub Option<String>);
impl Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = &self.0 {
            write!(f, "url('{}')", v)
        } else {
            Ok(())
        }
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
pub fn em<T: Into<f64>>(v: T) -> Em {
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
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    Rgba(Some((r, g, b, a)))
}
#[inline]
pub fn hex<T: Into<String>>(v: T) -> Hex {
    Hex(Some(v.into()))
}
#[inline]
pub fn hsl(h: u16, s: u8, l: u8) -> Hsl {
    Hsl(Some((h, s, l)))
}
#[inline]
pub fn url<T: Into<String>>(v: T) -> Url {
    Url(Some(v.into()))
}
