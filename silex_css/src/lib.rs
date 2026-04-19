pub mod builder;
pub mod properties;
pub mod runtime;
pub mod theme;
pub mod types;

pub mod prelude {
    pub use crate::builder::{Style, sty};
    pub use crate::runtime::{DynamicCss, DynamicStyleManager, inject_style};
    pub use crate::theme::{ThemeVariables, set_global_theme, theme_variables, use_theme, ThemeToCss};
    pub use crate::types::*;
}

pub use runtime::{DynamicCss, DynamicStyleManager, inject_style, make_dynamic_val_for};
