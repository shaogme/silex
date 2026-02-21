use crate::css::inject_style;
use crate::css::types::{ValidFor, props};
use silex_core::traits::{Get, IntoSignal, With};
use silex_dom::attribute::{ApplyTarget, ApplyToDom, IntoStorable};
use std::fmt::Display;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;
use wasm_bindgen::JsCast;

pub(crate) type DynamicValue = Rc<dyn Fn() -> String>;
pub(crate) type StaticRule = (&'static str, String);
pub(crate) type DynamicRule = (&'static str, DynamicValue);
pub(crate) type PseudoRule = (&'static str, Style);

pub struct Style {
    pub(crate) static_rules: Vec<StaticRule>,
    pub(crate) dynamic_rules: Vec<DynamicRule>,
    pub(crate) pseudo_rules: Vec<PseudoRule>,
}

impl Default for Style {
    fn default() -> Self {
        Self::new()
    }
}

impl Style {
    pub fn new() -> Self {
        Self {
            static_rules: Vec::new(),
            dynamic_rules: Vec::new(),
            pseudo_rules: Vec::new(),
        }
    }

    pub fn on_hover(mut self, f: impl FnOnce(Style) -> Style) -> Self {
        self.pseudo_rules.push((":hover", f(Style::new())));
        self
    }

    pub fn on_active(mut self, f: impl FnOnce(Style) -> Style) -> Self {
        self.pseudo_rules.push((":active", f(Style::new())));
        self
    }

    pub fn on_focus(mut self, f: impl FnOnce(Style) -> Style) -> Self {
        self.pseudo_rules.push((":focus", f(Style::new())));
        self
    }

    pub fn pseudo(mut self, class: &'static str, f: impl FnOnce(Style) -> Style) -> Self {
        self.pseudo_rules.push((class, f(Style::new())));
        self
    }
}

pub fn sty() -> Style {
    Style::new()
}

macro_rules! implement_css_properties {
    ( $( ($prop_snake:ident, $prop_kebab:expr, $type_struct:ty) ),* $(,)? ) => {
        impl Style {
            $(
                pub fn $prop_snake<V, ValType>(mut self, value: V) -> Self
                where
                    V: IntoSignal<Value = ValType> + 'static,
                    ValType: ValidFor<$type_struct> + Display + Clone + 'static,
                    <V as IntoSignal>::Signal: Get + 'static,
                    <<V as IntoSignal>::Signal as With>::Value: Display,
                {
                    if value.is_constant_value() {
                        let signal = value.into_signal();
                        let val_str = format!("{}", signal.get());
                        self.static_rules.push(($prop_kebab, val_str));
                    } else {
                        let signal = value.into_signal();
                        self.dynamic_rules.push(($prop_kebab, Rc::new(move || format!("{}", signal.get()))));
                    }
                    self
                }
            )*
        }
    };
}

implement_css_properties! {
    (width, "width", props::Width),
    (height, "height", props::Height),
    (margin, "margin", props::Margin),
    (padding, "padding", props::Padding),
    (color, "color", props::Color),
    (background_color, "background-color", props::BackgroundColor),
    (z_index, "z-index", props::ZIndex),
    (display, "display", props::Display),
    (position, "position", props::Position),
    (flex_direction, "flex-direction", props::FlexDirection),
    (background_image, "background-image", props::BackgroundImage),

    (border, "border", props::Border),
    (border_width, "border-width", props::BorderWidth),
    (border_style, "border-style", props::BorderStyle),
    (border_color, "border-color", props::BorderColor),
    (border_radius, "border-radius", props::BorderRadius),
    (font_size, "font-size", props::FontSize),
    (cursor, "cursor", props::Cursor),
    (gap, "gap", props::Gap),
}

impl ApplyToDom for Style {
    fn apply(self, el: &web_sys::Element, _target: ApplyTarget) {
        if !self.static_rules.is_empty()
            || !self
                .pseudo_rules
                .iter()
                .all(|(_, s)| s.static_rules.is_empty())
        {
            let mut hasher = DefaultHasher::new();

            for (k, v) in &self.static_rules {
                k.hash(&mut hasher);
                v.hash(&mut hasher);
            }

            for (pseudo, style) in &self.pseudo_rules {
                pseudo.hash(&mut hasher);
                for (k, v) in &style.static_rules {
                    k.hash(&mut hasher);
                    v.hash(&mut hasher);
                }
            }

            let hash_val = hasher.finish();
            let class_name = format!("slx-bldr-{:x}", hash_val);

            let mut css_str = String::new();

            if !self.static_rules.is_empty() {
                css_str.push_str(&format!(".{} {{\n", class_name));
                for (k, v) in &self.static_rules {
                    css_str.push_str(&format!("  {}: {};\n", k, v));
                }
                css_str.push_str("}\n");
            }

            for (pseudo, style) in &self.pseudo_rules {
                if !style.static_rules.is_empty() {
                    css_str.push_str(&format!(".{}{} {{\n", class_name, pseudo));
                    for (k, v) in &style.static_rules {
                        css_str.push_str(&format!("  {}: {};\n", k, v));
                    }
                    css_str.push_str("}\n");
                }
            }

            if !css_str.is_empty() {
                inject_style(&class_name, &css_str);
                let _ = el.class_list().add_1(&class_name);
            }
        }

        for (prop, getter) in self.dynamic_rules {
            let el_clone = el.clone();

            silex_core::reactivity::Effect::new(move |_| {
                let v = getter();
                if let Some(style) = el_clone
                    .dyn_ref::<web_sys::HtmlElement>()
                    .map(|e| e.style())
                    .or_else(|| el_clone.dyn_ref::<web_sys::SvgElement>().map(|e| e.style()))
                {
                    let _ = style.set_property(prop, &v);
                }
            });
        }

        let dyn_pseudo: Vec<_> = self
            .pseudo_rules
            .into_iter()
            .filter(|(_, s)| !s.dynamic_rules.is_empty())
            .map(|(p, s)| (p, s.dynamic_rules))
            .collect();

        if !dyn_pseudo.is_empty() {
            std::thread_local! {
                static INSTANCE_COUNTER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
            }
            let instance_id = INSTANCE_COUNTER.with(|c| {
                let id = c.get();
                c.set(id + 1);
                id
            });
            let dyn_class = format!("slx-bldr-dyn-{}", instance_id);
            let _ = el.class_list().add_1(&dyn_class);

            let manager = std::rc::Rc::new(crate::css::DynamicStyleManager::new(&dyn_class));

            silex_core::reactivity::Effect::new(move |_| {
                let mut combined_css = String::new();
                for (pseudo, rules) in &dyn_pseudo {
                    combined_css.push_str(&format!(".{}{} {{\n", dyn_class, pseudo));
                    for (prop, getter) in rules {
                        let val = getter();
                        combined_css.push_str(&format!("  {}: {};\n", prop, val));
                    }
                    combined_css.push_str("}\n");
                }
                manager.update(&combined_css);
            });
        }
    }
}

impl IntoStorable for Style {
    type Stored = Self;
    fn into_storable(self) -> Self::Stored {
        self
    }
}
