use crate::SilexError;
use crate::dom::attribute::AttributeValue;
use crate::dom::tags::Tag;
use crate::dom::view::View;
use crate::reactivity::{RwSignal, create_effect, on_cleanup};

use super::tags;
use std::marker::PhantomData;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::Element as WebElem;

/// Identity function to wrap text content as a View.
/// This matches the API expected by the showcase example and provides a explicit way to denote text nodes.
pub fn text<V: View>(content: V) -> V {
    content
}

/// 基础 DOM 元素包装器
#[derive(Clone)]
pub struct Element {
    pub dom_element: WebElem,
}

pub fn mount_to_body<V: View>(view: V) {
    let document = crate::dom::document();
    let body = document.body().expect("No body element");

    // Create a root reactive scope to ensure context and effects work correctly
    crate::reactivity::create_scope(move || {
        view.mount(&body);
    });
}

impl Element {
    pub fn new(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self { dom_element }
    }

    pub fn new_svg(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
            .expect("Failed to create SVG element");
        Self { dom_element }
    }

    // --- 统一的属性 API ---

    pub fn attr(self, name: &str, value: impl AttributeValue) -> Self {
        value.apply(&self.dom_element, name);
        self
    }

    pub fn id(self, value: impl AttributeValue) -> Self {
        self.attr("id", value)
    }

    pub fn class(self, value: impl AttributeValue) -> Self {
        self.attr("class", value)
    }

    pub fn style(self, value: impl AttributeValue) -> Self {
        self.attr("style", value)
    }

    // --- 事件 API ---

    pub fn on_click<F>(self, callback: F) -> Self
    where
        F: Fn(web_sys::MouseEvent) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            callback(e);
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .dom_element
            .add_event_listener_with_callback("click", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.dom_element.clone();
        let js_fn = js_value.clone();

        // 注册清理回调
        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("click", &js_fn);
            drop(closure);
        });

        self
    }

    pub fn on_input<F>(self, mut callback: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                let input = target.unchecked_into::<web_sys::HtmlInputElement>();
                callback(input.value());
            } else {
                let err = SilexError::Dom("Input event has no target".into());
                crate::error::handle_error(err);
            }
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .dom_element
            .add_event_listener_with_callback("input", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.dom_element.clone();
        let js_fn = js_value.clone();

        // 注册清理回调
        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("input", &js_fn);
            drop(closure);
        });

        self
    }

    pub fn bind_value(self, signal: RwSignal<String>) -> Self {
        let this = self.on_input(move |value| {
            signal.set(value);
        });

        let dom_element = this.dom_element.clone();

        create_effect(move || {
            let value = signal.get();
            if let Some(input) = dom_element.dyn_ref::<web_sys::HtmlInputElement>() {
                if input.value() != value {
                    input.set_value(&value);
                }
            } else if let Some(area) = dom_element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                if area.value() != value {
                    area.set_value(&value);
                }
            } else if let Some(select) = dom_element.dyn_ref::<web_sys::HtmlSelectElement>() {
                if select.value() != value {
                    select.set_value(&value);
                }
            } else {
                let _ = dom_element.set_attribute("value", &value);
            }
        });

        this
    }

    // --- 统一的子节点/文本 API ---

    pub fn child<V: View>(self, view: V) -> Self {
        view.mount(&self.dom_element);
        self
    }
}

impl std::ops::Deref for Element {
    type Target = WebElem;

    fn deref(&self) -> &Self::Target {
        &self.dom_element
    }
}

/// Type-safe wrapper for DOM elements
#[derive(Clone)]
pub struct TypedElement<T> {
    pub element: Element,
    _marker: PhantomData<T>,
}

impl<T> TypedElement<T> {
    pub(crate) fn new(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self {
            element: Element { dom_element },
            _marker: PhantomData,
        }
    }

    pub(crate) fn new_svg(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
            .expect("Failed to create SVG element");
        Self {
            element: Element { dom_element },
            _marker: PhantomData,
        }
    }

    pub fn into_untyped(self) -> Element {
        self.element
    }

    // --- Unified Attribute API ---

    pub fn attr(self, name: &str, value: impl AttributeValue) -> Self {
        value.apply(&self.element, name);
        self
    }

    pub fn id(self, value: impl AttributeValue) -> Self {
        self.attr("id", value)
    }

    pub fn class(self, value: impl AttributeValue) -> Self {
        self.attr("class", value)
    }

    pub fn style(self, value: impl AttributeValue) -> Self {
        self.attr("style", value)
    }

    // --- Event API (Duplicated from Element) ---

    pub fn on_click<F>(self, callback: F) -> Self
    where
        F: Fn(web_sys::MouseEvent) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            callback(e);
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .element
            .add_event_listener_with_callback("click", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.element.clone();
        let js_fn = js_value.clone();

        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("click", &js_fn);
            drop(closure);
        });

        self
    }

    pub fn on_input<F>(self, mut callback: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |e: web_sys::InputEvent| {
            if let Some(target) = e.target() {
                let input = target.unchecked_into::<web_sys::HtmlInputElement>();
                callback(input.value());
            } else {
                let err = SilexError::Dom("Input event has no target".into());
                crate::error::handle_error(err);
            }
        }) as Box<dyn FnMut(_)>);

        let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
        if let Err(e) = self
            .element
            .add_event_listener_with_callback("input", js_value)
            .map_err(SilexError::from)
        {
            crate::error::handle_error(e);
            return self;
        }

        let target = self.element.clone();
        let js_fn = js_value.clone();

        on_cleanup(move || {
            let _ = target.remove_event_listener_with_callback("input", &js_fn);
            drop(closure);
        });

        self
    }

    pub fn bind_value(self, signal: RwSignal<String>) -> Self {
        let this = self.on_input(move |value| {
            signal.set(value);
        });

        let dom_element = this.element.dom_element.clone();

        create_effect(move || {
            let value = signal.get();
            if let Some(input) = dom_element.dyn_ref::<web_sys::HtmlInputElement>() {
                if input.value() != value {
                    input.set_value(&value);
                }
            } else if let Some(area) = dom_element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                if area.value() != value {
                    area.set_value(&value);
                }
            } else if let Some(select) = dom_element.dyn_ref::<web_sys::HtmlSelectElement>() {
                if select.value() != value {
                    select.set_value(&value);
                }
            } else {
                let _ = dom_element.set_attribute("value", &value);
            }
        });

        this
    }

    // --- Children API ---

    pub fn child<V: View>(self, view: V) -> Self {
        view.mount(&self.element);
        self
    }
}

impl<T: Tag> Into<Element> for TypedElement<T> {
    fn into(self) -> Element {
        self.into_untyped()
    }
}

impl<T: Tag> std::ops::Deref for TypedElement<T> {
    type Target = Element;
    fn deref(&self) -> &Self::Target {
        &self.element
    }
}

pub mod tag {
    use super::tags::*;
    use super::*;
    use crate::dom::view::View;

    // --- Macros for boiler-plate reduction ---

    macro_rules! define_container {
        ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
            pub fn $fn_name<V: View>(child: V) -> TypedElement<$tag_type> {
                TypedElement::new($tag_str).child(child)
            }
        };
    }

    macro_rules! define_void {
        ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
            pub fn $fn_name() -> TypedElement<$tag_type> {
                TypedElement::new($tag_str)
            }
        };
    }

    macro_rules! define_svg_container {
        ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
            pub fn $fn_name<V: View>(child: V) -> TypedElement<$tag_type> {
                TypedElement::new_svg($tag_str).child(child)
            }
        };
    }

    macro_rules! define_svg_void {
        ($fn_name:ident, $tag_type:ident, $tag_str:expr) => {
            pub fn $fn_name() -> TypedElement<$tag_type> {
                TypedElement::new_svg($tag_str)
            }
        };
    }

    // --- HTML Containers ---
    // Structure & Text
    define_container!(div, Div, "div");
    define_container!(span, Span, "span");
    define_container!(p, P, "p");
    define_container!(h1, H1, "h1");
    define_container!(h2, H2, "h2");
    define_container!(h3, H3, "h3");
    define_container!(h4, H4, "h4");
    define_container!(h5, H5, "h5");
    define_container!(h6, H6, "h6");

    // Layout & Semantics
    define_container!(header, Header, "header");
    define_container!(footer, Footer, "footer");
    define_container!(main, Main, "main");
    define_container!(section, Section, "section");
    define_container!(article, Article, "article");
    define_container!(aside, Aside, "aside");
    define_container!(nav, Nav, "nav");
    define_container!(address, Address, "address");

    // Lists
    define_container!(ul, Ul, "ul");
    define_container!(ol, Ol, "ol");
    define_container!(li, Li, "li");

    // Inline & Formatting
    define_container!(a, A, "a");
    define_container!(button, Button, "button");
    define_container!(label, Label, "label");
    define_container!(pre, Pre, "pre");
    define_container!(code, Code, "code");
    define_container!(blockquote, Blockquote, "blockquote");
    define_container!(em, Em, "em");
    define_container!(strong, Strong, "strong");
    define_container!(s, S, "s");
    define_container!(time, Time, "time");
    define_container!(figure, Figure, "figure");
    define_container!(figcaption, Figcaption, "figcaption");

    // Forms
    define_container!(form, Form, "form");
    define_container!(select, Select, "select");
    define_container!(textarea, Textarea, "textarea");

    pub fn option<V: View>(child: V) -> TypedElement<OptionTag> {
        TypedElement::new("option").child(child)
    }

    // Table
    define_container!(table, Table, "table");
    define_container!(thead, Thead, "thead");
    define_container!(tbody, Tbody, "tbody");
    define_container!(tr, Tr, "tr");
    define_container!(td, Td, "td");

    // --- HTML Void Elements (No Children) ---
    define_void!(input, Input, "input");
    define_void!(img, Img, "img");
    define_void!(br, Br, "br");
    define_void!(hr, Hr, "hr");
    define_void!(link, Link, "link");

    // --- SVG Containers ---
    define_svg_container!(svg, Svg, "svg");
    define_svg_container!(g, G, "g");
    define_svg_container!(defs, Defs, "defs");
    define_svg_container!(filter, Filter, "filter");

    // --- SVG Voids (Shapes & Primitives) ---
    // Treating shapes as void for cleaner API (use attributes for definition)
    define_svg_void!(path, Path, "path");
    define_svg_void!(rect, Rect, "rect");
    define_svg_void!(circle, Circle, "circle");
    define_svg_void!(line, Line, "line");
    define_svg_void!(polyline, Polyline, "polyline");
    define_svg_void!(polygon, Polygon, "polygon");

    // Filter Primitives
    define_svg_void!(fe_turbulence, FeTurbulence, "feTurbulence");
    define_svg_void!(
        fe_component_transfer,
        FeComponentTransfer,
        "feComponentTransfer"
    );
    define_svg_void!(fe_func_r, FeFuncR, "feFuncR");
    define_svg_void!(fe_func_g, FeFuncG, "feFuncG");
    define_svg_void!(fe_func_b, FeFuncB, "feFuncB");
    define_svg_void!(fe_gaussian_blur, FeGaussianBlur, "feGaussianBlur");
    define_svg_void!(
        fe_specular_lighting,
        FeSpecularLighting,
        "feSpecularLighting"
    );
    define_svg_void!(fe_point_light, FePointLight, "fePointLight");
    define_svg_void!(fe_composite, FeComposite, "feComposite");
    define_svg_void!(fe_displacement_map, FeDisplacementMap, "feDisplacementMap");
}

// --- Macros for Tag DSL ---

/// Internal macro to generate public macros for tags.
/// We use $d to pass dollar signs to the inner macro.
#[macro_export]
macro_rules! define_tag_macros {
    ($($name:ident),+; $d:tt) => {
        $(
            #[macro_export]
            macro_rules! $name {
                () => {
                    $crate::dom::element::tag::$name(())
                };
                ($d($d child:expr),+ $d(,)?) => {
                    $crate::dom::element::tag::$name(($d($d child),+))
                };
            }
        )*
    };
}

// Generate macros for all container tags
define_tag_macros!(
    div, span, p, h1, h2, h3, h4, h5, h6,
    header, footer, main, section, article, aside, nav, address,
    ul, ol, li,
    a, button, label, pre, code, blockquote, em, strong, s, time, figure, figcaption,
    form, select, textarea, option,
    table, thead, tbody, tr, td,
    // SVG Containers
    svg, g, defs, filter
    ; $
);
