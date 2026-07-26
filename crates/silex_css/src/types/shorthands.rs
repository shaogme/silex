use crate::types::{ValidFor, props, units::impl_string_value_wrapper};
use std::fmt::Display;

// ==========================================
// 复合属性工厂 (Shorthand Factories)
// ==========================================

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BorderValue(String);
impl_string_value_wrapper!(BorderValue);

pub fn border<W, S, C>(width: W, style: S, color: C) -> BorderValue
where
    W: ValidFor<props::BorderWidth> + Display,
    S: ValidFor<props::BorderStyle> + Display,
    C: ValidFor<props::BorderColor> + Display,
{
    BorderValue(format!("{} {} {}", width, style, color))
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MarginValue(String);
impl_string_value_wrapper!(MarginValue);
impl ValidFor<props::Margin> for MarginValue {}

/// `margin` 简写的构造器。
///
/// 这里**只**产出 `margin` 这一个属性的值。单边写法请用 builder 上的
/// `margin_top()` / `margin_right()` / …——此前这里有一组 `margin::top(v)`，
/// 它约束的是定位属性 `props::Top`、返回的却是 `ValidFor<props::Margin>` 的
/// `MarginValue`，于是 `sty().margin(margin::top(px(4)))` 实际产出的是
/// `margin: 4px`（**四边**）。名字说的是上边，效果是四边。
pub mod margin {
    use super::*;
    pub fn all<T: ValidFor<props::Margin> + Display>(v: T) -> MarginValue {
        MarginValue(format!("{}", v))
    }

    /// 两值写法：`margin: <block> <inline>`。
    ///
    /// CSS 的两值 margin 是「**纵向 横向**」。此前这个函数叫 `x_y(x, y)`，
    /// 按笛卡尔坐标的直觉，`x` 该是横向——但它落在第一位，实际作用于上下。
    /// 参数名与效果正好颠倒，且产物合法，不会有任何报错。
    /// 换成 CSS 自己的说法（block = 纵向，inline = 横向）就不会再有歧义。
    pub fn block_inline<B, I>(block: B, inline: I) -> MarginValue
    where
        B: ValidFor<props::Margin> + Display,
        I: ValidFor<props::Margin> + Display,
    {
        MarginValue(format!("{} {}", block, inline))
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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaddingValue(String);
impl_string_value_wrapper!(PaddingValue);
impl ValidFor<props::Padding> for PaddingValue {}

/// `padding` 简写的构造器。语义与 [`margin`] 完全对应。
pub mod padding {
    use super::*;
    pub fn all<T: ValidFor<props::Padding> + Display>(v: T) -> PaddingValue {
        PaddingValue(format!("{}", v))
    }

    /// 两值写法：`padding: <block> <inline>`（纵向 横向）。
    pub fn block_inline<B, I>(block: B, inline: I) -> PaddingValue
    where
        B: ValidFor<props::Padding> + Display,
        I: ValidFor<props::Padding> + Display,
    {
        PaddingValue(format!("{} {}", block, inline))
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
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlexValue(String);
impl_string_value_wrapper!(FlexValue);
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
pub struct TransitionValue(String);
impl_string_value_wrapper!(TransitionValue);
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
pub struct BackgroundValue(String);
impl_string_value_wrapper!(BackgroundValue);
impl ValidFor<props::Background> for BackgroundValue {}

pub fn background<C, I>(color: C, image: I) -> BackgroundValue
where
    C: ValidFor<props::BackgroundColor> + Display,
    I: ValidFor<props::BackgroundImage> + Display,
{
    BackgroundValue(format!("{} {}", color, image))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::units::px;

    /// 报告 P3-1：两值 margin 是「纵向 横向」，而此前的 `x_y(x, y)` 把第一位
    /// 叫做 `x`——参数名与效果正好颠倒，且产物合法、不会报错。
    #[test]
    fn the_two_value_form_is_block_then_inline() {
        assert_eq!(margin::block_inline(px(1), px(2)).to_string(), "1px 2px");
        assert_eq!(padding::block_inline(px(8), px(16)).to_string(), "8px 16px");
    }

    #[test]
    fn the_four_value_form_keeps_css_order() {
        assert_eq!(
            margin::top_right_bottom_left(px(1), px(2), px(3), px(4)).to_string(),
            "1px 2px 3px 4px"
        );
    }

    #[test]
    fn the_single_value_form_covers_every_side() {
        assert_eq!(margin::all(px(4)).to_string(), "4px");
    }
}
