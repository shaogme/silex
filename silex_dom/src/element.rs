use crate::attribute::{ApplyTarget, ApplyToDom, IntoStorable, PendingAttribute};
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
pub fn text<V: crate::view::MountRef>(content: V) -> V {
    content
}

/// 基础 DOM 元素包装器
#[derive(Clone, PartialEq)]
pub struct Element {
    pub dom_element: WebElem,
}

pub fn mount_to_body<V: crate::view::Mount>(view: V) {
    let document = crate::document();
    let body = document.body().expect("No body element");
    let node: web_sys::Node = body.into();

    // Create a root reactive scope to ensure context and effects work correctly
    silex_core::reactivity::create_scope(move || {
        view.mount(&node, Vec::new());
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

impl crate::view::ApplyAttributes for Element {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        let consolidated = crate::attribute::consolidate_attributes(attrs);
        for attr in consolidated {
            attr.apply(&self.dom_element);
        }
    }
}

impl crate::view::Mount for Element {
    fn mount(mut self, parent: &::web_sys::Node, attrs: Vec<PendingAttribute>) {
        if !attrs.is_empty() {
            crate::view::ApplyAttributes::apply_attributes(&mut self, attrs);
        }

        if let Err(e) = parent
            .append_child(&self.dom_element)
            .map_err(SilexError::from)
        {
            silex_core::error::handle_error(e);
        }
    }
}

impl crate::view::MountRef for Element {
    fn mount_ref(&self, parent: &::web_sys::Node, attrs: Vec<PendingAttribute>) {
        if !attrs.is_empty() {
            let consolidated = crate::attribute::consolidate_attributes(attrs);
            for attr in consolidated {
                attr.apply(&self.dom_element);
            }
        }

        if let Err(e) = parent
            .append_child(&self.dom_element)
            .map_err(SilexError::from)
        {
            silex_core::error::handle_error(e);
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
pub struct TypedElement<T> {
    pub element: Element,
    _marker: PhantomData<T>,
}

impl<T> Clone for TypedElement<T> {
    fn clone(&self) -> Self {
        Self {
            element: self.element.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> PartialEq for TypedElement<T> {
    fn eq(&self, other: &Self) -> bool {
        self.element == other.element
    }
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

impl<T> crate::view::ApplyAttributes for TypedElement<T> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute>) {
        self.element.apply_attributes(attrs);
    }
}

impl<T> crate::view::Mount for TypedElement<T> {
    fn mount(mut self, parent: &::web_sys::Node, attrs: Vec<PendingAttribute>) {
        if !attrs.is_empty() {
            crate::view::ApplyAttributes::apply_attributes(&mut self, attrs);
        }

        if let Err(e) = parent.append_child(&self.element).map_err(SilexError::from) {
            silex_core::error::handle_error(e);
        }
    }
}

impl<T> crate::view::MountRef for TypedElement<T> {
    fn mount_ref(&self, parent: &::web_sys::Node, attrs: Vec<PendingAttribute>) {
        self.element.mount_ref(parent, attrs);
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
    let handler = callback.into_handler();
    let type_str = event.name().to_string();
    bind_event_impl(dom_element, type_str, handler);
}

/// 内部实现：只针对事件类型 E 进行单态化，去除了对闭包类型 F 的依赖。
/// 这样全应用所有同类型的事件（如 Click）将共享这段机器码。
pub fn bind_event_impl<E>(dom_element: &WebElem, event_name: String, mut handler: Box<dyn FnMut(E)>)
where
    E: wasm_bindgen::convert::FromWasmAbi + 'static,
{
    let closure = Closure::wrap(Box::new(move |e: E| {
        handler(e);
    }) as Box<dyn FnMut(E)>);

    let js_value = closure.as_ref().unchecked_ref::<js_sys::Function>();

    if let Err(e) = dom_element
        .add_event_listener_with_callback(&event_name, js_value)
        .map_err(SilexError::from)
    {
        silex_core::error::handle_error(e);
        return;
    }

    let target = dom_element.clone();
    let js_fn = js_value.clone();

    on_cleanup(move || {
        let _ = target.remove_event_listener_with_callback(&event_name, &js_fn);
        drop(closure);
    });
}
