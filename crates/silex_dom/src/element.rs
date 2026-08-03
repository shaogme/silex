use crate::attribute::{ApplyTarget, AttributeBuilder, IntoStorable, PendingAttribute};
use crate::event::{EventDescriptor, EventHandler};
use crate::view::{AnyView, ApplyAttributes, RootViewOwner, View, ViewOwner, ViewOwnerToken};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::Element as WebElem;

use silex_core::{RootScope, SilexError, error::handle_error};

pub mod tags;
pub use tags::*;

pub fn text<'scope, 'run, V: View<'scope, 'run>>(content: V) -> V {
    content
}

pub struct Element<'scope, 'run> {
    pub dom_element: WebElem,
    pub(crate) pending_attrs: Vec<PendingAttribute<'scope, 'run>>,
    pub(crate) children: Vec<AnyView<'scope, 'run>>,
}

impl<'scope, 'run> Clone for Element<'scope, 'run> {
    fn clone(&self) -> Self {
        Self {
            dom_element: self.dom_element.clone(),
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
        }
    }
}

impl PartialEq for Element<'_, '_> {
    fn eq(&self, other: &Self) -> bool {
        self.dom_element == other.dom_element
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope, 'run> Element<'scope, 'run> {
    pub fn new(tag: &str) -> Self {
        let document = crate::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element");
        Self {
            dom_element,
            pending_attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn new_svg(tag: &str) -> Self {
        let document = crate::document();
        let dom_element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
            .expect("Failed to create SVG element");
        Self {
            dom_element,
            pending_attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_child<V>(tag: &str, child: V) -> Self
    where
        V: View<'scope, 'run> + 'scope,
    {
        let mut element = Self::new(tag);
        element.children.push(child.into_any());
        element
    }

    fn all_attrs(
        &self,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) -> Vec<PendingAttribute<'scope, 'run>> {
        let mut all = self.pending_attrs.clone();
        all.extend(attrs);
        crate::attribute::consolidate_attributes(all)
    }

    fn mount_inner(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        let token = owner.token();
        for attr in self.all_attrs(attrs) {
            attr.apply(&self.dom_element, &token);
        }
        if let Err(error) = parent
            .append_child(&self.dom_element)
            .map_err(SilexError::from)
        {
            handle_error(error);
            return;
        }
        for child in &self.children {
            child.mount(owner, self.dom_element.as_ref(), Vec::new());
        }
    }
}

impl<'scope, 'run> AttributeBuilder<'scope, 'run> for Element<'scope, 'run> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope, 'run>,
    {
        self.pending_attrs
            .push(PendingAttribute::build(value.into_storable(), target));
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<E::EventType, M> + Clone + 'static,
    {
        self.pending_attrs
            .push(PendingAttribute::new_scoped(move |element, owner| {
                bind_event(element, event, callback.clone(), owner)
            }));
        self
    }
}

impl<'scope, 'run> ApplyAttributes<'scope, 'run> for Element<'scope, 'run> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        self.pending_attrs = crate::attribute::consolidate_attributes({
            let mut current = std::mem::take(&mut self.pending_attrs);
            current.extend(attrs);
            current
        });
    }
}

impl<'scope, 'run> View<'scope, 'run> for Element<'scope, 'run> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        self.mount_inner(owner, parent, attrs);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        self.mount_inner(owner, parent, attrs);
    }
}

/// Mount a root view using the caller-owned root scope.
pub fn mount_to_body<V>(root_scope: &RootScope, view: V)
where
    V: View<'static, 'static> + 'static,
{
    let document = crate::document();
    let body = document.body().expect("No body element");
    let node: web_sys::Node = body.into();
    let owner = RootViewOwner::new(root_scope.clone());
    view.mount_owned(&owner, &node, Vec::new());
}

pub struct TypedElement<'scope, 'run, T: Tag> {
    pub dom_element: T::DomElement,
    pub(crate) pending_attrs: Vec<PendingAttribute<'scope, 'run>>,
    pub(crate) children: Vec<AnyView<'scope, 'run>>,
    marker: PhantomData<T>,
}

impl<'scope, 'run, T: Tag> Clone for TypedElement<'scope, 'run, T> {
    fn clone(&self) -> Self {
        Self {
            dom_element: self.dom_element.clone(),
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: Tag> PartialEq for TypedElement<'_, '_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_element() == other.as_element()
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope, 'run, T: Tag> TypedElement<'scope, 'run, T> {
    pub fn new(tag: &str) -> Self {
        let document = crate::document();
        let dom_element = document
            .create_element(tag)
            .expect("Failed to create element")
            .unchecked_into::<T::DomElement>();
        Self {
            dom_element,
            pending_attrs: Vec::new(),
            children: Vec::new(),
            marker: PhantomData,
        }
    }

    pub fn new_svg(tag: &str) -> Self {
        let document = crate::document();
        let dom_element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), tag)
            .expect("Failed to create SVG element")
            .unchecked_into::<T::DomElement>();
        Self {
            dom_element,
            pending_attrs: Vec::new(),
            children: Vec::new(),
            marker: PhantomData,
        }
    }

    pub fn with_child<V>(tag: &str, child: V) -> Self
    where
        V: View<'scope, 'run> + 'scope,
    {
        let mut element = Self::new(tag);
        element.children.push(child.into_any());
        element
    }

    #[inline(always)]
    pub fn as_element(&self) -> &web_sys::Element {
        AsRef::<web_sys::Element>::as_ref(&self.dom_element)
    }

    #[inline(always)]
    pub fn as_node(&self) -> &web_sys::Node {
        AsRef::<web_sys::Node>::as_ref(&self.dom_element)
    }

    pub fn into_untyped(self) -> Element<'scope, 'run> {
        Element {
            dom_element: self.as_element().clone(),
            pending_attrs: self.pending_attrs,
            children: self.children,
        }
    }
}

impl<'scope, 'run, T: Tag> AttributeBuilder<'scope, 'run> for TypedElement<'scope, 'run, T> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope, 'run>,
    {
        self.pending_attrs
            .push(PendingAttribute::build(value.into_storable(), target));
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<E::EventType, M> + Clone + 'static,
    {
        self.pending_attrs
            .push(PendingAttribute::new_scoped(move |element, owner| {
                bind_event(element, event, callback.clone(), owner)
            }));
        self
    }
}

impl<'scope, 'run, T: Tag> ApplyAttributes<'scope, 'run> for TypedElement<'scope, 'run, T> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope, 'run>>) {
        let mut current = std::mem::take(&mut self.pending_attrs);
        current.extend(attrs);
        self.pending_attrs = crate::attribute::consolidate_attributes(current);
    }
}

impl<'scope, 'run, T: Tag> View<'scope, 'run> for TypedElement<'scope, 'run, T> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) {
        let token = owner.token();
        let mut all_attrs = self.pending_attrs.clone();
        all_attrs.extend(attrs);
        for attr in crate::attribute::consolidate_attributes(all_attrs) {
            attr.apply(self.as_element(), &token);
        }
        if let Err(error) = parent
            .append_child(self.as_node())
            .map_err(SilexError::from)
        {
            handle_error(error);
            return;
        }
        for child in &self.children {
            child.mount(owner, self.as_node(), Vec::new());
        }
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope, 'run>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope, 'run>>,
    ) where
        Self: Sized,
    {
        self.mount(owner, parent, attrs);
    }
}

impl<'scope, 'run, T: Tag> From<TypedElement<'scope, 'run, T>> for Element<'scope, 'run> {
    fn from(value: TypedElement<'scope, 'run, T>) -> Self {
        value.into_untyped()
    }
}

impl<'scope, 'run> std::ops::Deref for Element<'scope, 'run> {
    type Target = WebElem;

    fn deref(&self) -> &Self::Target {
        &self.dom_element
    }
}

impl<'scope, 'run, T: Tag> std::ops::Deref for TypedElement<'scope, 'run, T> {
    type Target = WebElem;

    fn deref(&self) -> &Self::Target {
        self.dom_element.as_ref()
    }
}

pub fn bind_event<'scope, 'run, E, F, M>(
    dom_element: &WebElem,
    event: E,
    callback: F,
    owner: &ViewOwnerToken<'scope, 'run>,
) where
    E: crate::event::EventDescriptor + 'static,
    F: EventHandler<E::EventType, M>,
{
    let handler = callback.into_handler();
    bind_event_impl(dom_element, event.name().to_string(), handler, owner);
}

pub fn bind_event_impl<'scope, 'run, E>(
    dom_element: &WebElem,
    event_name: String,
    mut handler: Box<dyn FnMut(E)>,
    owner: &ViewOwnerToken<'scope, 'run>,
) where
    E: wasm_bindgen::convert::FromWasmAbi + 'static,
{
    let closure = Closure::wrap(Box::new(move |event: E| handler(event)) as Box<dyn FnMut(E)>);
    let js_fn = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
    if let Err(error) = dom_element
        .add_event_listener_with_callback(&event_name, closure.as_ref().unchecked_ref())
        .map_err(SilexError::from)
    {
        handle_error(error);
        return;
    }

    let target = dom_element.clone();
    let event_name_for_cleanup = event_name.clone();
    let closure = Rc::new(RefCell::new(Some(closure)));
    let closure_for_cleanup = closure.clone();
    owner.on_cleanup(Box::new(move || {
        let _ = target.remove_event_listener_with_callback(&event_name_for_cleanup, &js_fn);
        let _ = closure_for_cleanup.borrow_mut().take();
    }));
}
