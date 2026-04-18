use crate::types::{AngleMark, CalcValue, CssColor, ValidFor, props};
use std::fmt::Display;

// ==========================================
// 渐变 DSL (Gradients)
// ==========================================

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GradientValue(pub Option<String>);
impl Display for GradientValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(v) = &self.0 {
            write!(f, "{}", v)
        } else {
            Ok(())
        }
    }
}
impl ValidFor<props::BackgroundImage> for GradientValue {}
impl ValidFor<props::Background> for GradientValue {}

#[derive(Clone, Debug, PartialEq)]
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

use crate::types::units::Deg;
impl From<Deg> for Direction {
    fn from(v: Deg) -> Self {
        Self::Angle(v.into())
    }
}

#[derive(Clone, Debug, PartialEq)]
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
        GradientValue(Some(s))
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
        GradientValue(Some(s))
    }
}

pub fn radial_gradient() -> RadialGradientBuilder {
    RadialGradientBuilder::new()
}
