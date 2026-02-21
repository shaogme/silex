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

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Overrides {
    #[serde(default)]
    pub whitelist: Vec<String>,
    #[serde(default)]
    pub groups: HashMap<String, String>,
    #[serde(default)]
    pub keywords: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CssConfig {
    pub properties: Vec<ProcessedProp>,
    #[serde(default)]
    pub syntaxes: HashMap<String, MdnCssSyntax>,
}

impl CssConfig {
    pub fn apply_overrides(&mut self, overrides: &Overrides) {
        for prop in &mut self.properties {
            // Apply group overrides
            if let Some(group_str) = overrides.groups.get(&prop.name) {
                prop.group = match group_str.as_str() {
                    "Dimension" => PropGroup::Dimension,
                    "Color" => PropGroup::Color,
                    "Number" => PropGroup::Number,
                    "Keyword" => PropGroup::Keyword,
                    "Shorthand" => PropGroup::Shorthand,
                    _ => PropGroup::Custom,
                };
            }
            // Apply keyword overrides
            if let Some(keywords) = overrides.keywords.get(&prop.name) {
                prop.keywords = keywords.clone();
            }
        }
    }
}
