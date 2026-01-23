use crate::SilexError;
use crate::dom::attribute::AttributeValue;
use crate::dom::tags::Tag;
use crate::dom::view::View;
use crate::reactivity::{RwSignal, create_effect, on_cleanup};

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

    // --- Advanced Class Helpers ---

    pub fn class_toggle<C>(self, name: &str, condition: C) -> Self
    where
        (String, C): AttributeValue,
    {
        // Construct a tuple (String, C) which implements AttributeValue
        self.attr("class", (name.to_string(), condition))
    }

    pub fn classes<V>(self, value: V) -> Self
    where
        V: AttributeValue,
    {
        // Alias for class(), emphasizing multiple classes or complex logic
        self.attr("class", value)
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
    pub fn new(tag: &str) -> Self {
        let document = crate::dom::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self {
            element: Element { dom_element },
            _marker: PhantomData,
        }
    }

    pub fn new_svg(tag: &str) -> Self {
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

    pub fn class_toggle<C>(self, name: &str, condition: C) -> Self
    where
        (String, C): AttributeValue,
    {
        self.attr("class", (name.to_string(), condition))
    }

    pub fn classes<V>(self, value: V) -> Self
    where
        V: AttributeValue,
    {
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

// End of core element logic
