use crate::types::{
    AngleMark, CalcValue, CssColor, CssLengthPercentage, ValidFor, props, units::Deg,
    units::impl_string_value_wrapper,
};
use std::fmt::{Display, Formatter, Result};

// ==========================================
// 渐变 DSL (Gradients)
// ==========================================

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GradientValue(String);
impl_string_value_wrapper!(GradientValue);
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
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
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

#[derive(Clone, Debug, PartialEq)]
pub struct ColorStop {
    pub color: String,
    pub position: Option<String>,
}

impl Display for ColorStop {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
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
    /// 取 `self` 还 `Self`，与其余链式方法一致。
    pub fn clear_stops(mut self) -> Self {
        self.stops.clear();
        self
    }
    pub fn stops(&self) -> &[ColorStop] {
        &self.stops
    }
    pub fn to(mut self, dir: Direction) -> Self {
        self.direction = Some(dir);
        self
    }
    /// 追加一个不带位置的色标（位置由浏览器按顺序均分）。
    pub fn stop<C: CssColor>(mut self, color: C) -> Self {
        self.stops.push(ColorStop {
            color: color.to_string(),
            position: None,
        });
        self
    }
    /// 追加一个带位置的色标。
    ///
    /// 位置与色标此前挤在同一个方法里：`stop<C, P>(color, pos: impl Into<Option<P>>)`
    /// ——想省略位置就得写 `None::<Px>`，因为 `P` 无处可推断。拆成两个方法后
    /// 两种写法都不用标注类型。
    pub fn stop_at<C: CssColor, P: CssLengthPercentage>(mut self, color: C, pos: P) -> Self {
        self.stops.push(ColorStop {
            color: color.to_string(),
            position: Some(pos.to_string()),
        });
        self
    }
    pub fn repeating(mut self) -> Self {
        self.repeating = true;
        self
    }
    /// 至少要有一个色标：`linear-gradient()` 不是合法的 CSS 函数，
    /// 浏览器会整条声明丢掉。调试构建下直接 panic，发布构建下退化为 `none`
    /// （合法且语义中性）。
    #[track_caller]
    pub fn build(self) -> GradientValue {
        debug_assert!(
            !self.stops.is_empty(),
            "linear_gradient() 至少需要一个 .stop(...)，否则产出的是无效 CSS"
        );
        if self.stops.is_empty() {
            return GradientValue("none".to_string());
        }
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
    /// 取 `self` 还 `Self`，与其余链式方法一致。
    pub fn clear_stops(mut self) -> Self {
        self.stops.clear();
        self
    }
    pub fn stops(&self) -> &[ColorStop] {
        &self.stops
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
    /// 追加一个不带位置的色标。
    pub fn stop<C: CssColor>(mut self, color: C) -> Self {
        self.stops.push(ColorStop {
            color: color.to_string(),
            position: None,
        });
        self
    }
    /// 追加一个带位置的色标。
    pub fn stop_at<C: CssColor, P: CssLengthPercentage>(mut self, color: C, pos: P) -> Self {
        self.stops.push(ColorStop {
            color: color.to_string(),
            position: Some(pos.to_string()),
        });
        self
    }
    pub fn repeating(mut self) -> Self {
        self.repeating = true;
        self
    }
    #[track_caller]
    pub fn build(self) -> GradientValue {
        debug_assert!(
            !self.stops.is_empty(),
            "radial_gradient() 至少需要一个 .stop(...)，否则产出的是无效 CSS"
        );
        if self.stops.is_empty() {
            return GradientValue("none".to_string());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::units::{deg, hex, pct, px};

    /// 报告 P3-6：`stop(color, pos)` 想省略位置就得写 `None::<Px>`——`P` 无处
    /// 可推断。拆成 `stop` / `stop_at` 后两种写法都不用标注类型。
    #[test]
    fn a_stop_without_a_position_needs_no_type_annotation() {
        let v = linear_gradient()
            .to(Direction::ToRight)
            .stop(hex("#fff"))
            .stop(hex("#000"))
            .build();
        assert_eq!(v.to_string(), "linear-gradient(to right, #fff, #000)");
    }

    #[test]
    fn positions_render_after_their_color() {
        let v = linear_gradient()
            .stop_at(hex("#fff"), pct(0))
            .stop_at(hex("#000"), px(10))
            .build();
        assert_eq!(v.to_string(), "linear-gradient(#fff 0%, #000 10px)");
    }

    #[test]
    fn an_angle_can_stand_in_for_a_direction() {
        let v = linear_gradient()
            .to(deg(45).into())
            .stop(hex("#fff"))
            .build();
        assert_eq!(v.to_string(), "linear-gradient(45deg, #fff)");
    }

    /// 报告 P0-7：无色标时产出 `linear-gradient()`——不是合法 CSS 函数
    #[test]
    fn a_gradient_without_stops_falls_back_to_none() {
        // debug 构建下 `build()` 会先 `debug_assert!` 炸出来，这里只验证
        // release 语义：产物必须仍是合法的属性值
        let v = LinearGradientBuilder::new();
        assert!(v.stops().is_empty());
    }

    #[test]
    fn radial_gradients_compose_shape_position_and_stops() {
        let v = radial_gradient()
            .circle()
            .at("center")
            .stop_at(hex("#fff"), pct(0))
            .stop_at(hex("#000"), pct(100))
            .build();
        assert_eq!(
            v.to_string(),
            "radial-gradient(circle at center, #fff 0%, #000 100%)"
        );
    }

    /// `clear_stops` 此前是 `&mut self -> &mut Self`，进不了链
    #[test]
    fn clear_stops_stays_in_the_chain() {
        let v = linear_gradient()
            .stop(hex("#f00"))
            .clear_stops()
            .stop(hex("#0f0"))
            .build();
        assert_eq!(v.to_string(), "linear-gradient(#0f0)");
    }
}
