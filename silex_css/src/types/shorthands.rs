use crate::types::{ValidFor, props};
use std::fmt::Display;

// ==========================================
// 复合属性工厂 (Shorthand Factories)
// ==========================================

#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
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

#[derive(Clone, Debug, Default, PartialEq)]
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
