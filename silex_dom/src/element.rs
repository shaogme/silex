use crate::attribute::{ApplyTarget, ApplyToDom, IntoStorable, PendingAttribute};
use crate::view::View;
use silex_core::SilexError;
use silex_core::reactivity::on_cleanup;

use std::marker::PhantomData;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::Element as WebElem;

pub mod tags;
use crate::event::{EventDescriptor, EventHandler};
pub use tags::*;

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
        view.mount(&body, Vec::new());
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
    fn mount(mut self, parent: &::web_sys::Node, attrs: Vec<PendingAttribute>) {
        if !attrs.is_empty() {
            self.apply_attributes(attrs);
        }

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

    fn into_any(self) -> crate::view::AnyView {
        crate::view::AnyView::Element(self.clone())
    }

    fn into_shared(self) -> crate::view::SharedView {
        crate::view::SharedView::Element(self)
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
    fn mount(mut self, parent: &::web_sys::Node, attrs: Vec<PendingAttribute>) {
        if !attrs.is_empty() {
            self.apply_attributes(attrs);
        }

        if let Err(e) = parent.append_child(&self.element).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
        }
    }

    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        self.element.apply_attributes(attrs);
    }

    fn into_any(self) -> crate::view::AnyView {
        crate::view::AnyView::Element(self.element.clone())
    }

    fn into_shared(self) -> crate::view::SharedView {
        crate::view::SharedView::Element(self.element)
    }
}

impl<T: Tag> From<TypedElement<T>> for Element {
    fn from(val: TypedElement<T>) -> Self {
        val.into_untyped()
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
