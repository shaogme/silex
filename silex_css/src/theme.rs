use silex_core::prelude::*;
use std::fmt::Display;

/// A trait that every Silex Theme must implement.
/// This allows the `styled!` macro to perform compile-time type checks.
/// Usually implemented via the `define_theme!` macro.
pub trait ThemeType {}

/// A dummy theme type to satisfy the default macro requirements.
/// Users should alias this to their actual theme or use #[theme(MyTheme)].
pub type Theme = ();

pub trait ThemeToCss: Display {
    fn to_css_variables(&self) -> String;
    fn get_variable_values(&self) -> Vec<String>;
    fn get_variable_names() -> &'static [&'static str];
}

/// Helper that applies theme variables to any element without an extra wrapper.
/// Usage: `div(children).apply(theme_variables(theme))`
pub fn theme_variables<T>(theme: ReadSignal<T>) -> ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + Clone + 'static,
{
    // Provide the theme signal in the current reactive scope
    ::silex_core::prelude::provide_context(theme);
    ThemeVariables(theme)
}

/// A structure that can be applied to a DOM element to inject theme variables.
pub struct ThemeVariables<T>(pub ReadSignal<T>);

impl<T> ::silex_dom::attribute::ApplyToDom for ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + Clone + 'static,
{
    fn apply(&self, el: &::web_sys::Element, _target: ::silex_dom::attribute::ApplyTarget) {
        let theme = self.0;
        let el = el.clone();
        ::silex_core::prelude::Effect::new(move |prev_values: Option<Vec<String>>| {
            use ::wasm_bindgen::JsCast;
            if let Some(style) = el
                .dyn_ref::<::web_sys::HtmlElement>()
                .map(|e| e.style())
                .or_else(|| el.dyn_ref::<::web_sys::SvgElement>().map(|e| e.style()))
            {
                let theme_val = theme.get();
                let names = T::get_variable_names();
                let values = theme_val.get_variable_values();

                if let Some(old_vals) = &prev_values {
                    for (i, (name, value)) in names.iter().zip(values.iter()).enumerate() {
                        if Some(value) != old_vals.get(i) {
                            let _ = style.set_property(name, value);
                        }
                    }
                } else {
                    for (name, value) in names.iter().zip(values.iter()) {
                        let _ = style.set_property(name, value);
                    }
                }
                return values;
            }
            Vec::new()
        });
    }
}

impl<T> silex_dom::attribute::IntoStorable for ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + Clone + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

/// Hook to get the current theme signal from context.
pub fn use_theme<T: 'static>() -> ReadSignal<T> {
    ::silex_core::prelude::use_context::<ReadSignal<T>>()
        .expect("No ThemeProvider found in hierarchy")
}

/// Sets a global theme that applies to the entire document (:root).
pub fn set_global_theme<T>(theme: ReadSignal<T>)
where
    T: ThemeType + ThemeToCss + Clone + 'static,
{
    // Register the theme in the global context as well
    ::silex_core::prelude::provide_context(theme);

    // Apply reactive updates to :root
    ::silex_core::prelude::Effect::new(move |prev_values: Option<Vec<String>>| {
        use ::wasm_bindgen::JsCast;
        let doc = ::silex_dom::document();
        if let Some(root) = doc.document_element()
            && let Some(style) = root.dyn_ref::<::web_sys::HtmlElement>().map(|e| e.style())
        {
            let theme_val = theme.get();
            let names = T::get_variable_names();
            let values = theme_val.get_variable_values();

            if let Some(old_vals) = &prev_values {
                for (i, (name, value)) in names.iter().zip(values.iter()).enumerate() {
                    if Some(value) != old_vals.get(i) {
                        let _ = style.set_property(name, value);
                    }
                }
            } else {
                for (name, value) in names.iter().zip(values.iter()) {
                    let _ = style.set_property(name, value);
                }
            }
            return values;
        }
        Vec::new()
    });
}
