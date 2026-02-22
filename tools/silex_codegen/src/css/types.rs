use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MdnCssProperty {
    pub syntax: String,
    pub status: String,
    pub inherited: bool,
    #[serde(default)]
    pub groups: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MdnCssSyntax {
    pub syntax: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PropGroup {
    Dimension,
    Color,
    Number,
    Keyword,
    Shorthand,
    Custom,
    Complex,
    Alpha,
}

impl PropGroup {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dimension => "Dimension",
            Self::Color => "Color",
            Self::Number => "Number",
            Self::Keyword => "Keyword",
            Self::Shorthand => "Shorthand",
            Self::Custom => "Custom",
            Self::Complex => "Complex",
            Self::Alpha => "Alpha",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedProp {
    pub name: String,        // e.g. "background-color"
    pub method_name: String, // e.g. "background_color"
    pub struct_name: String, // e.g. "BackgroundColor"
    pub group: PropGroup,
    pub keywords: Vec<String>, // For Keyword group
}

use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CssConfig {
    pub properties: Vec<ProcessedProp>,
    #[serde(default)]
    pub syntaxes: HashMap<String, MdnCssSyntax>,
}
