use crate::attribute::{ApplyTarget, ApplyToDom, IntoStorable, PendingAttribute};
use crate::view::View;
use silex_core::SilexError;
use silex_core::node_ref::NodeRef;
use silex_core::reactivity::{Effect, RwSignal, on_cleanup};
use silex_core::traits::{Get, Set};

use std::marker::PhantomData;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::Element as WebElem;

pub mod tags;
use crate::event::{EventDescriptor, EventHandler};
pub use tags::*;

// --- Macros for Deduplication ---

macro_rules! impl_element_common {
    () => {
        pub fn class_toggle<C>(self, name: &str, condition: C) -> Self
        where
            (String, C): ApplyToDom,
        {
            (name.to_string(), condition).apply(&self.as_web_element(), ApplyTarget::Class);
            self
        }

        pub fn classes<V>(self, value: V) -> Self
        where
            V: ApplyToDom,
        {
            value.apply(&self.as_web_element(), ApplyTarget::Class);
            self
        }

        pub fn node_ref<N>(self, node_ref: NodeRef<N>) -> Self
        where
            N: JsCast + Clone + 'static,
        {
            let el = self.as_web_element();
            if let Ok(typed) = el.dyn_into::<N>() {
                node_ref.load(typed);
            } else {
                silex_core::log::console_error("NodeRef type mismatch: failed to cast element");
            }
            self
        }

        // --- Event API ---

        pub fn on_click<F, M>(self, callback: F) -> Self
        where
            F: EventHandler<web_sys::MouseEvent, M>,
        {
            let mut handler = callback.into_handler();
            let closure = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
                handler(e);
            }) as Box<dyn FnMut(_)>);

            let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
            let dom_element = self.as_web_element();

            if let Err(e) = dom_element
                .add_event_listener_with_callback("click", js_value)
                .map_err(SilexError::from)
            {
                silex_core::error::handle_error(e);
                return self;
            }

            let target = dom_element.clone();
            let js_fn = js_value.clone();

            on_cleanup(move || {
                let _ = target.remove_event_listener_with_callback("click", &js_fn);
                drop(closure);
            });

            self
        }

        pub fn on_input<F, M>(self, callback: F) -> Self
        where
            F: EventHandler<String, M>,
        {
            let mut handler = callback.into_handler();
            let closure = Closure::wrap(Box::new(move |e: web_sys::InputEvent| {
                if let Some(target) = e.target() {
                    let input = target.unchecked_into::<web_sys::HtmlInputElement>();
                    handler(input.value());
                } else {
                    let err = SilexError::Dom("Input event has no target".into());
                    silex_core::error::handle_error(err);
                }
            }) as Box<dyn FnMut(_)>);

            let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
            let dom_element = self.as_web_element();

            if let Err(e) = dom_element
                .add_event_listener_with_callback("input", js_value)
                .map_err(SilexError::from)
            {
                silex_core::error::handle_error(e);
                return self;
            }

            let target = dom_element.clone();
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

            let dom_element = this.as_web_element();

            Effect::new(move |_| {
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

        pub fn on_untyped<E, F>(self, event_type: &str, mut callback: F) -> Self
        where
            E: wasm_bindgen::convert::FromWasmAbi + 'static,
            F: FnMut(E) + 'static,
        {
            let closure = Closure::wrap(Box::new(move |e: E| {
                callback(e);
            }) as Box<dyn FnMut(E)>);

            let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();
            let dom_element = self.as_web_element();

            if let Err(e) = dom_element
                .add_event_listener_with_callback(event_type, js_value)
                .map_err(SilexError::from)
            {
                silex_core::error::handle_error(e);
                return self;
            }

            let target = dom_element.clone();
            let js_fn = js_value.clone();
            let type_clone = event_type.to_string();

            on_cleanup(move || {
                let _ = target.remove_event_listener_with_callback(&type_clone, &js_fn);
                drop(closure);
            });

            self
        }
    };
}

/// Identity function to wrap text content as a View.
/// This matches the API expected by the showcase example and provides a explicit way to denote text nodes.
pub fn text<V: View>(content: V) -> V {
    content
}

/// 基础 DOM 元素包装器
#[derive(Clone, PartialEq)]
pub struct Element {
    pub dom_element: WebElem,
}

pub fn mount_to_body<V: View>(view: V) {
    let document = crate::document();
    let body = document.body().expect("No body element");

    // Create a root reactive scope to ensure context and effects work correctly
    silex_core::reactivity::create_scope(move || {
        view.mount(&body);
    });
}

impl Element {
    pub fn new(tag: &str) -> Self {
        let document = crate::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self { dom_element }
    }

    pub fn new_svg(tag: &str) -> Self {
        let document = crate::document();
        let dom_element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
            .expect("Failed to create SVG element");
        Self { dom_element }
    }

    fn as_web_element(&self) -> WebElem {
        self.dom_element.clone()
    }

    // --- 统一的属性/事件 API (Generated) ---
    impl_element_common!();
}

// --- AttributeBuilder Implementation ---

use crate::attribute::AttributeBuilder;

impl AttributeBuilder for Element {
    fn build_attribute<V>(self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable,
    {
        // Convert to storable type, then apply to DOM
        value.into_storable().apply(&self.dom_element, target);
        self
    }

    fn build_event<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<E::EventType, M> + Clone + 'static,
    {
        bind_event(&self.dom_element, event, callback);
        self
    }
}

impl View for Element {
    fn mount(self, parent: &::web_sys::Node) {
        if let Err(e) = parent
            .append_child(&self.dom_element)
            .map_err(SilexError::from)
        {
            silex_core::error::handle_error(e);
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        for attr in attrs {
            attr.apply(&self.dom_element);
        }
    }
}

impl std::ops::Deref for Element {
    type Target = WebElem;

    fn deref(&self) -> &Self::Target {
        &self.dom_element
    }
}

/// Type-safe wrapper for DOM elements
#[derive(Clone, PartialEq)]
pub struct TypedElement<T> {
    pub element: Element,
    _marker: PhantomData<T>,
}

impl<T> TypedElement<T> {
    pub fn new(tag: &str) -> Self {
        let document = crate::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self {
            element: Element { dom_element },
            _marker: PhantomData,
        }
    }

    pub fn new_svg(tag: &str) -> Self {
        let document = crate::document();
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

    fn as_web_element(&self) -> WebElem {
        self.element.dom_element.clone()
    }

    // --- Unified Attribute API (Generated) ---
    impl_element_common!();
}

impl<T> AttributeBuilder for TypedElement<T> {
    fn build_attribute<V>(self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable,
    {
        // Convert to storable type, then apply to DOM
        value
            .into_storable()
            .apply(&self.element.dom_element, target);
        self
    }

    fn build_event<E, F, M>(self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<E::EventType, M> + Clone + 'static,
    {
        bind_event(&self.element.dom_element, event, callback);
        self
    }
}

impl<T> View for TypedElement<T> {
    fn mount(self, parent: &::web_sys::Node) {
        if let Err(e) = parent.append_child(&self.element).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        self.element.apply_attributes(attrs);
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

/// Helper function to bind an event to a DOM element.
/// Used by Element's `.on()` and Component's forwarded `.on()`.
pub fn bind_event<E, F, M>(dom_element: &WebElem, event: E, callback: F)
where
    E: crate::event::EventDescriptor + 'static,
    F: EventHandler<E::EventType, M>,
{
    let mut handler = callback.into_handler();
    let type_str = event.name();

    let closure = Closure::wrap(Box::new(move |e: E::EventType| {
        handler(e);
    }) as Box<dyn FnMut(E::EventType)>);

    let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();

    // Note: event.name() returns generic string, we need to pass str reference
    let type_str_ref: &str = &type_str;
    if let Err(e) = dom_element
        .add_event_listener_with_callback(type_str_ref, js_value)
        .map_err(SilexError::from)
    {
        silex_core::error::handle_error(e);
        return;
    }

    let target = dom_element.clone();
    let js_fn = js_value.clone();
    // We need to own the string for the cleanup closure
    let type_clone = type_str.to_string();

    on_cleanup(move || {
        let _ = target.remove_event_listener_with_callback(&type_clone, &js_fn);
        drop(closure);
    });
}
