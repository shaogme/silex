pub mod codegen;
pub mod fetch;
pub mod types;

pub use codegen::{generate_keywords_code, generate_registry_macro};
pub use fetch::fetch_and_merge_css;
use std::fs;
use std::path::Path;
pub use types::CssConfig;

pub fn load_config(path: &Path) -> Result<CssConfig, Box<dyn std::error::Error>> {
    if !path.exists() {
        return Ok(CssConfig { properties: vec![] });
    }

    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(CssConfig { properties: vec![] });
    }

    Ok(serde_json::from_str(&content)?)
}
