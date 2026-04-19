use silex_core::prelude::*;
use std::fmt::Display;

/// A trait that every Silex Theme must implement.
/// This allows the `styled!` macro to perform compile-time type checks.
/// Usually implemented via the `theme!` macro.
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
pub fn theme_variables<T>(theme: impl IntoSignal<Value = T> + 'static) -> ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
{
    let signal = theme.into_signal();
    // Provide the theme signal in the current reactive scope
    ::silex_core::prelude::provide_context(signal);
    ThemeVariables(signal)
}

/// A structure that can be applied to a DOM element to inject theme variables.
pub struct ThemeVariables<T>(pub Signal<T>);

impl<T> ::silex_dom::attribute::ApplyToDom for ThemeVariables<T>
where
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
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
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}

/// Hook to get the current theme signal from context.
pub fn use_theme<T: 'static>() -> Signal<T> {
    ::silex_core::prelude::use_context::<Signal<T>>().expect("No ThemeProvider found in hierarchy")
}

/// Sets a global theme that applies to the entire document (:root).
pub fn set_global_theme<T>(theme: impl IntoSignal<Value = T> + 'static)
where
    T: ThemeType + ThemeToCss + RxCloneData + 'static,
{
    let signal = theme.into_signal();
    // Register the theme in the global context as well
    ::silex_core::prelude::provide_context(signal);

    // Apply reactive updates to :root
    ::silex_core::prelude::Effect::new(move |prev_values: Option<Vec<String>>| {
        use ::wasm_bindgen::JsCast;
        let doc = ::silex_dom::document();
        if let Some(root) = doc.document_element()
            && let Some(style) = root.dyn_ref::<::web_sys::HtmlElement>().map(|e| e.style())
        {
            let theme_val = signal.get();
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

/// A trait for theme patches that only override a subset of variables.
pub trait ThemePatchToCss {
    /// Returns a list of (variable_name, value).
    /// If the value is None, the variable should be removed from the local element style (enabling inheritance).
    fn get_patch_entries(&self) -> Vec<(&'static str, Option<String>)>;
}

/// Helper that applies a theme patch to an element.
/// This allows for granular overrides while relying on CSS variable inheritance for the rest.
pub fn theme_patch<P>(patch: impl IntoSignal<Value = P> + 'static) -> ThemePatchVariables<P>
where
    P: ThemePatchToCss + RxCloneData + 'static,
{
    ThemePatchVariables(patch.into_signal())
}

/// A structure that can be applied to a DOM element to inject theme patch variables.
pub struct ThemePatchVariables<P>(pub Signal<P>);

impl<P> ::silex_dom::attribute::ApplyToDom for ThemePatchVariables<P>
where
    P: ThemePatchToCss + RxCloneData + 'static,
{
    fn apply(&self, el: &::web_sys::Element, _target: ::silex_dom::attribute::ApplyTarget) {
        let patch = self.0;
        let el = el.clone();
        ::silex_core::prelude::Effect::new(move |prev_values: Option<Vec<Option<String>>>| {
            use ::wasm_bindgen::JsCast;
            if let Some(style) = el
                .dyn_ref::<::web_sys::HtmlElement>()
                .map(|e| e.style())
                .or_else(|| el.dyn_ref::<::web_sys::SvgElement>().map(|e| e.style()))
            {
                let patch_val = patch.get();
                let entries = patch_val.get_patch_entries();
                let current_values: Vec<Option<String>> =
                    entries.iter().map(|(_, v)| v.clone()).collect();

                if let Some(old_vals) = &prev_values {
                    for (i, (name, value)) in entries.into_iter().enumerate() {
                        if Some(&value) != old_vals.get(i) {
                            match value {
                                Some(v) => {
                                    let _ = style.set_property(name, &v);
                                }
                                None => {
                                    let _ = style.remove_property(name);
                                }
                            }
                        }
                    }
                } else {
                    for (name, value) in entries {
                        match value {
                            Some(v) => {
                                let _ = style.set_property(name, &v);
                            }
                            None => {
                                let _ = style.remove_property(name);
                            }
                        }
                    }
                }
                return current_values;
            }
            Vec::new()
        });
    }
}

impl<P> silex_dom::attribute::IntoStorable for ThemePatchVariables<P>
where
    P: ThemePatchToCss + RxCloneData + 'static,
{
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
