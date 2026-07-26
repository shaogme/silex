use crate::types::{
    CssAngle, CssLength, CssLengthPercentage, ValidFor, props,
    units::{finite, impl_string_value_wrapper},
};
use std::fmt::{Display, Formatter, Result, Write};

// ==========================================
// 复杂属性 DSL (Complex Properties)
// ==========================================

// --- Transform ---

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformValue(String);
impl_string_value_wrapper!(TransformValue);
impl ValidFor<props::Transform> for TransformValue {}

/// `transform` 的链式构造器。
///
/// 每个函数的参数都按 CSS 规范约束到对应的值类型：位移收 `<length-percentage>`
/// （`translateZ` / `perspective` 只收 `<length>`），旋转与倾斜收 `<angle>`，
/// 缩放与矩阵收 `<number>`。此前所有参数只约束 `Display`，于是
/// `transform().translate(hex("#fff"), rgb(1,2,3)).rotate("banana")` 是合法
/// Rust——与整个 crate 的「强类型 CSS」定位割裂。
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TransformBuilder {
    parts: Vec<String>,
}

/// 变换函数里的 `<number>`。
///
/// 单独走一遍 `finite` 是因为 `scale(f64::NAN)` 会产出 `scale(NaN)`——
/// 无效声明，浏览器整条丢弃。
#[inline]
#[track_caller]
fn num(v: impl Into<f64>) -> f64 {
    finite(v.into())
}

impl TransformBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 清空已累积的变换函数。
    ///
    /// 取 `self` 还 `Self`，与其余链式方法一致——此前它是
    /// `clear(&mut self) -> &mut Self`，在一条链里根本用不了。
    pub fn clear(mut self) -> Self {
        self.parts.clear();
        self
    }

    pub fn parts(&self) -> &[String] {
        &self.parts
    }

    fn push(mut self, part: String) -> Self {
        self.parts.push(part);
        self
    }

    // --- 位移 ---

    pub fn translate<X: CssLengthPercentage, Y: CssLengthPercentage>(self, x: X, y: Y) -> Self {
        self.push(format!("translate({}, {})", x, y))
    }

    pub fn translate_x<V: CssLengthPercentage>(self, v: V) -> Self {
        self.push(format!("translateX({})", v))
    }

    pub fn translate_y<V: CssLengthPercentage>(self, v: V) -> Self {
        self.push(format!("translateY({})", v))
    }

    /// `translateZ()` 只接受 `<length>`——百分比在这里是无效的。
    pub fn translate_z<V: CssLength>(self, v: V) -> Self {
        self.push(format!("translateZ({})", v))
    }

    pub fn translate_3d<X, Y, Z>(self, x: X, y: Y, z: Z) -> Self
    where
        X: CssLengthPercentage,
        Y: CssLengthPercentage,
        Z: CssLength,
    {
        self.push(format!("translate3d({}, {}, {})", x, y, z))
    }

    // --- 旋转 ---

    pub fn rotate<A: CssAngle>(self, angle: A) -> Self {
        self.push(format!("rotate({})", angle))
    }

    pub fn rotate_x<A: CssAngle>(self, angle: A) -> Self {
        self.push(format!("rotateX({})", angle))
    }

    pub fn rotate_y<A: CssAngle>(self, angle: A) -> Self {
        self.push(format!("rotateY({})", angle))
    }

    pub fn rotate_z<A: CssAngle>(self, angle: A) -> Self {
        self.push(format!("rotateZ({})", angle))
    }

    /// `rotate3d(x, y, z, <angle>)`——前三个是旋转轴向量的分量（纯数）。
    pub fn rotate_3d<A: CssAngle>(
        self,
        x: impl Into<f64>,
        y: impl Into<f64>,
        z: impl Into<f64>,
        angle: A,
    ) -> Self {
        self.push(format!(
            "rotate3d({}, {}, {}, {})",
            num(x),
            num(y),
            num(z),
            angle
        ))
    }

    // --- 缩放 ---

    pub fn scale(self, s: impl Into<f64>) -> Self {
        self.push(format!("scale({})", num(s)))
    }

    pub fn scale_x_y(self, x: impl Into<f64>, y: impl Into<f64>) -> Self {
        self.push(format!("scale({}, {})", num(x), num(y)))
    }

    pub fn scale_x(self, x: impl Into<f64>) -> Self {
        self.push(format!("scaleX({})", num(x)))
    }

    pub fn scale_y(self, y: impl Into<f64>) -> Self {
        self.push(format!("scaleY({})", num(y)))
    }

    pub fn scale_z(self, z: impl Into<f64>) -> Self {
        self.push(format!("scaleZ({})", num(z)))
    }

    pub fn scale_3d(self, x: impl Into<f64>, y: impl Into<f64>, z: impl Into<f64>) -> Self {
        self.push(format!("scale3d({}, {}, {})", num(x), num(y), num(z)))
    }

    // --- 倾斜 ---

    pub fn skew<X: CssAngle, Y: CssAngle>(self, x: X, y: Y) -> Self {
        self.push(format!("skew({}, {})", x, y))
    }

    pub fn skew_x<A: CssAngle>(self, angle: A) -> Self {
        self.push(format!("skewX({})", angle))
    }

    pub fn skew_y<A: CssAngle>(self, angle: A) -> Self {
        self.push(format!("skewY({})", angle))
    }

    // --- 矩阵与透视 ---

    /// `matrix(a, b, c, d, tx, ty)`——2D 仿射矩阵。
    pub fn matrix(self, m: [f64; 6]) -> Self {
        let parts: Vec<String> = m.iter().map(|v| num(*v).to_string()).collect();
        self.push(format!("matrix({})", parts.join(", ")))
    }

    /// `matrix3d(…)`——16 个分量，列主序。
    pub fn matrix_3d(self, m: [f64; 16]) -> Self {
        let parts: Vec<String> = m.iter().map(|v| num(*v).to_string()).collect();
        self.push(format!("matrix3d({})", parts.join(", ")))
    }

    /// `perspective()` 只接受 `<length>`。
    pub fn perspective<V: CssLength>(self, v: V) -> Self {
        self.push(format!("perspective({})", v))
    }

    /// 没有任何变换函数时产出 `none`，而不是空串。
    ///
    /// 空串会渲染成 `transform: ;`——无效声明，浏览器直接丢弃；`none` 是
    /// `transform` 的初始值，语义上正是「不做变换」。
    pub fn build(self) -> TransformValue {
        if self.parts.is_empty() {
            return TransformValue("none".to_string());
        }
        let mut val = String::new();
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                val.push(' ');
            }
            val.push_str(part);
        }
        TransformValue(val)
    }
}

impl Display for TransformBuilder {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        // `TransformBuilder` 本身就是合法的属性值（不必先 `.build()`），所以
        // 空构造器必须和 `build()` 给出同一个答案 `none`——否则
        // `sty().transform(transform())` 会产出 `transform: ;`
        if self.parts.is_empty() {
            return f.write_str("none");
        }
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_char(' ')?;
            }
            f.write_str(part)?;
        }
        Ok(())
    }
}

impl ValidFor<props::Transform> for TransformBuilder {}

pub fn transform() -> TransformBuilder {
    TransformBuilder::new()
}

// --- Grid Template Areas ---

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridTemplateAreasValue(String);
impl_string_value_wrapper!(GridTemplateAreasValue);
impl ValidFor<props::GridTemplateAreas> for GridTemplateAreasValue {}

pub fn grid_template_areas<I, S>(areas: I) -> GridTemplateAreasValue
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut val = String::new();
    for (i, s) in areas.into_iter().enumerate() {
        if i > 0 {
            val.push(' ');
        }
        // 用户串里的 `"` 必须转义，否则 `a"; color:red; x:"` 这种输入会闭合
        // 当前字符串、越出声明边界，把任意 CSS 注入进来
        val.push_str(&crate::escape::css_string(s.as_ref()));
    }
    GridTemplateAreasValue(val)
}

// --- Font Variation Settings ---

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontVariationSettingsValue(String);
impl_string_value_wrapper!(FontVariationSettingsValue);
impl ValidFor<props::FontVariationSettings> for FontVariationSettingsValue {}

pub fn font_variation_settings<I, K, V>(settings: I) -> FontVariationSettingsValue
where
    I: IntoIterator<Item = (K, V)>,
    K: Display,
    V: Display,
{
    let mut val = String::new();
    for (i, (k, v)) in settings.into_iter().enumerate() {
        if i > 0 {
            val.push_str(", ");
        }
        val.push_str(&crate::escape::css_string(&k.to_string()));
        write!(val, " {}", v).unwrap();
    }
    FontVariationSettingsValue(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::units::{deg, pct, px, rad, rem, turn};

    /// 报告里的注入用例：值里的 `"` 曾能闭合当前字符串、越出声明边界
    #[test]
    fn grid_template_areas_cannot_break_out_of_the_declaration() {
        let v = grid_template_areas(["a\"; color:red; x:\""]).to_string();
        assert_eq!(v, r#""a\"; color:red; x:\"""#);
        // 只有首尾两个引号是“裸”的，中间那对已被转义 —— 值出不去声明边界
        let bare_quotes = v
            .char_indices()
            .filter(|&(i, c)| c == '"' && (i == 0 || v.as_bytes()[i - 1] != b'\\'))
            .count();
        assert_eq!(bare_quotes, 2, "{v}");
    }

    #[test]
    fn font_variation_settings_escapes_its_axis_names() {
        let v = font_variation_settings([("wght\"", 700)]).to_string();
        assert_eq!(v, r#""wght\"" 700"#);
    }

    #[test]
    fn empty_transform_builds_a_valid_value() {
        // 曾产出空串 → `transform: ;`
        assert_eq!(transform().build().to_string(), "none");
        // 不经 build() 直接当属性值用时也一样
        assert_eq!(transform().to_string(), "none");
        assert_eq!(
            transform().translate_x(px(1)).build().to_string(),
            "translateX(1px)"
        );
    }

    /// 报告 P3-5：所有参数只约束 `Display`，`rotate("banana")` 合法。
    /// 现在每个函数都按 CSS 规范收对应的值类型——这条测试能编译就说明
    /// 各个类型对上了，`trybuild` 反例负责证明错误类型进不来。
    #[test]
    fn every_transform_function_takes_its_own_value_type() {
        let v = transform()
            .translate(px(1), pct(50))
            .translate_x(rem(1))
            .translate_y(px(2))
            .translate_z(px(3))
            .translate_3d(px(1), pct(10), px(2))
            .rotate(deg(45))
            .rotate_x(rad(1.0))
            .rotate_y(turn(0.25))
            .rotate_z(deg(90))
            .rotate_3d(1, 0, 0, deg(30))
            .scale(1.05)
            .scale_x_y(2, 3)
            .scale_x(1)
            .scale_y(1)
            .scale_z(1)
            .scale_3d(1, 1, 1)
            .skew(deg(1), deg(2))
            .skew_x(deg(3))
            .skew_y(deg(4))
            .matrix([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
            .perspective(px(500))
            .build()
            .to_string();

        assert!(v.starts_with("translate(1px, 50%) translateX(1rem)"), "{v}");
        assert!(v.contains("rotate3d(1, 0, 0, 30deg)"), "{v}");
        assert!(v.contains("scale(1.05)"), "{v}");
        assert!(v.contains("matrix(1, 0, 0, 1, 0, 0)"), "{v}");
        assert!(v.ends_with("perspective(500px)"), "{v}");
    }

    #[test]
    fn matrix_3d_takes_all_sixteen_components() {
        let m = [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];
        assert_eq!(
            transform().matrix_3d(m).build().to_string(),
            "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)"
        );
    }

    /// `clear` 此前是 `&mut self -> &mut Self`，进不了链
    #[test]
    fn clear_stays_in_the_chain() {
        let v = transform().rotate(deg(90)).clear().scale(2).build();
        assert_eq!(v.to_string(), "scale(2)");
    }
}
