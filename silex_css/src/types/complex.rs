use crate::types::{ValidFor, props};
use std::fmt::{Display, Write};

// ==========================================
// 复杂属性 DSL (Complex Properties)
// ==========================================

// --- Transform ---

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransformValue(pub String);
impl Display for TransformValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ValidFor<props::Transform> for TransformValue {}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TransformBuilder {
    parts: Vec<String>,
}

impl TransformBuilder {
    pub fn new() -> Self {
        Self::default()
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

    pub fn build(self) -> TransformValue {
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
pub struct GridTemplateAreasValue(pub String);
impl Display for GridTemplateAreasValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
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
        write!(val, "\"{}\"", s.as_ref()).unwrap();
    }
    GridTemplateAreasValue(val)
}

// --- Font Variation Settings ---

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontVariationSettingsValue(pub String);
impl Display for FontVariationSettingsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
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
        write!(val, "\"{}\" {}", k, v).unwrap();
    }
    FontVariationSettingsValue(val)
}
