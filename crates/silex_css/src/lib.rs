pub mod builder;
pub mod class;
pub mod codegen;
pub mod escape;
pub mod layers;
pub mod runtime;
pub mod source;
pub mod theme;
#[cfg(feature = "tw")]
pub mod tw;
pub mod types;

pub mod prelude {
    pub use crate::builder::{Style, sty};
    pub use crate::class::IntoClass;
    pub use crate::cx;
    #[cfg(feature = "tw")]
    pub use crate::declare_variants;
    pub use crate::runtime::{DynamicCss, DynamicStyleManager, inject_style};
    pub use crate::source::{CssSource, IntoCssReactive, IntoCssSource, StaticCssValue};
    pub use crate::theme::{
        ThemePatchToCss, ThemeToCss, ThemeType, ThemeVariables, set_global_theme, theme_patch,
        theme_variables,
    };
    #[cfg(feature = "tw")]
    pub use crate::tw::VariantSchema;
    #[cfg(feature = "tw")]
    pub use crate::tw::variants::UnknownVariantOption;
    pub use crate::types::*;
    // `#[macro_export]` 把这三个宏放在了 crate 根，所以上面那句 `types::*`
    // 只带得走函数版的 `min` / `max` / `clamp`，宏版要单独 re-export。
    pub use crate::{css_clamp, css_max, css_min};
}

pub use class::IntoClass;
pub use codegen::properties;
pub use runtime::{
    CssPart, DynamicCss, DynamicStyleManager, GlobalStyleBinding, GlobalStyleView,
    StaticStyleTemplate, StyledDynamicRule, StyledVariantBinding, StyledVariantGroup,
    dynamic_rule_class, dynamic_rule_class_with_static, inject_managed_dynamic_style, inject_style,
    make_property_val, render_static_template,
};
pub use source::{CssSource, IntoCssReactive, IntoCssSource, StaticCssValue, static_css_value};
pub use theme::{ThemePatchToCss, ThemeToCss, ThemeType};
#[cfg(feature = "tw")]
pub use tw::VariantSchema;
pub use types::CssProperty;
