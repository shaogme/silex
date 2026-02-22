use crate::types::{ValidFor, props};
use std::fmt::Display;

// ==========================================
// 复杂属性 DSL (Complex Properties)
// ==========================================

// --- Transform ---

#[derive(Clone, Debug, PartialEq)]
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
        TransformValue(self.parts.join(" "))
    }
}

impl Display for TransformBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join(" "))
    }
}

impl ValidFor<props::Transform> for TransformBuilder {}

pub fn transform() -> TransformBuilder {
    TransformBuilder::new()
}

// --- Grid Template Areas ---

#[derive(Clone, Debug, PartialEq)]
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
    let val = areas
        .into_iter()
        .map(|s| format!("\"{}\"", s.as_ref()))
        .collect::<Vec<_>>()
        .join(" ");
    GridTemplateAreasValue(val)
}

// --- Font Variation Settings ---

#[derive(Clone, Debug, PartialEq)]
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
    let val = settings
        .into_iter()
        .map(|(k, v)| format!("\"{}\" {}", k, v))
        .collect::<Vec<_>>()
        .join(", ");
    FontVariationSettingsValue(val)
}
