use crate::types::{ValidFor, props, units::impl_string_value_wrapper};
use std::fmt::{Display, Formatter, Result, Write};

// ==========================================
// 复杂属性 DSL (Complex Properties)
// ==========================================

// --- Transform ---

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformValue(String);
impl_string_value_wrapper!(TransformValue);
impl ValidFor<props::Transform> for TransformValue {}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TransformBuilder {
    parts: Vec<String>,
}

impl TransformBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) -> &mut Self {
        self.parts.clear();
        self
    }

    pub fn parts(&self) -> &[String] {
        &self.parts
    }

    pub fn translate<X: Display, Y: Display>(mut self, x: X, y: Y) -> Self {
        self.parts.push(format!("translate({}, {})", x, y));
        self
    }

    pub fn translate_x<V: Display>(mut self, v: V) -> Self {
        self.parts.push(format!("translateX({})", v));
        self
    }

    pub fn translate_y<V: Display>(mut self, v: V) -> Self {
        self.parts.push(format!("translateY({})", v));
        self
    }

    pub fn rotate<A: Display>(mut self, angle: A) -> Self {
        self.parts.push(format!("rotate({})", angle));
        self
    }

    pub fn scale<S: Display>(mut self, s: S) -> Self {
        self.parts.push(format!("scale({})", s));
        self
    }

    pub fn scale_x_y<X: Display, Y: Display>(mut self, x: X, y: Y) -> Self {
        self.parts.push(format!("scale({}, {})", x, y));
        self
    }

    pub fn skew<X: Display, Y: Display>(mut self, x: X, y: Y) -> Self {
        self.parts.push(format!("skew({}, {})", x, y));
        self
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
        assert_eq!(
            transform().translate_x("1px").build().to_string(),
            "translateX(1px)"
        );
    }
}
