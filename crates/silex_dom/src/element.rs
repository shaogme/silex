use crate::attribute::{ApplyTarget, AttributeBuilder, IntoStorable, PendingAttribute};
use crate::event::{EventDescriptor, EventHandler};
use crate::view::{AnyView, ApplyAttributes, ScopedViewOwner, View, ViewOwner, ViewOwnerToken};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue, convert::FromWasmAbi, prelude::*};
use web_sys::Element as WebElem;

use silex_core::{RuntimeInputs, Scope, SilexError, error::handle_error};

pub mod tags;
pub use tags::*;

pub fn text<'scope, V: View<'scope>>(content: V) -> V {
    content
}

pub struct Element<'scope> {
    pub dom_element: WebElem,
    pub(crate) pending_attrs: Vec<PendingAttribute<'scope>>,
    pub(crate) children: Vec<AnyView<'scope>>,
}

impl<'scope> Clone for Element<'scope> {
    fn clone(&self) -> Self {
        Self {
            dom_element: self.dom_element.clone(),
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
        }
    }
}

impl PartialEq for Element<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.dom_element == other.dom_element
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope> Element<'scope> {
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
        V: View<'scope> + 'scope,
    {
        let mut element = Self::new(tag);
        element.children.push(child.into_any());
        element
    }

    fn all_attrs(&self, attrs: Vec<PendingAttribute<'scope>>) -> Vec<PendingAttribute<'scope>> {
        let mut all = self.pending_attrs.clone();
        all.extend(attrs);
        crate::attribute::consolidate_attributes(all)
    }

    fn mount_inner(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        let token = owner.token();
        let attrs = self.all_attrs(attrs);
        let mut inputs = RuntimeInputs::new();
        for attr in &attrs {
            inputs.extend(&attr.runtime_inputs());
        }
        if let Err(error) = token.validate_inputs(&inputs) {
            handle_error(error);
            return;
        }
        for attr in attrs {
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

impl<'scope> AttributeBuilder<'scope> for Element<'scope> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.pending_attrs
            .push(PendingAttribute::build(value.into_storable(), target));
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, E::EventType, M> + Clone + 'scope,
    {
        self.pending_attrs
            .push(PendingAttribute::new_scoped(move |element, owner| {
                bind_event(element, event, callback.clone(), owner)
            }));
        self
    }
}

impl<'scope> ApplyAttributes<'scope> for Element<'scope> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        self.pending_attrs = crate::attribute::consolidate_attributes({
            let mut current = std::mem::take(&mut self.pending_attrs);
            current.extend(attrs);
            current
        });
    }
}

impl<'scope> View<'scope> for Element<'scope> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        self.mount_inner(owner, parent, attrs);
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        self.mount_inner(owner, parent, attrs);
    }
}

/// Mount a view using the caller-owned scope.
pub fn mount_to_body<'scope, V>(scope: &Scope<'scope>, view: V)
where
    V: View<'scope> + 'scope,
{
    let document = crate::document();
    let body = document.body().expect("No body element");
    let node: web_sys::Node = body.into();
    let owner = ScopedViewOwner::new(*scope);
    view.mount_owned(&owner, &node, Vec::new());
}

pub struct TypedElement<'scope, T: Tag> {
    pub dom_element: T::DomElement,
    pub(crate) pending_attrs: Vec<PendingAttribute<'scope>>,
    pub(crate) children: Vec<AnyView<'scope>>,
    marker: PhantomData<T>,
}

impl<'scope, T: Tag> Clone for TypedElement<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            dom_element: self.dom_element.clone(),
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: Tag> PartialEq for TypedElement<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_element() == other.as_element()
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope, T: Tag> TypedElement<'scope, T> {
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
        V: View<'scope> + 'scope,
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

    pub fn into_untyped(self) -> Element<'scope> {
        Element {
            dom_element: self.as_element().clone(),
            pending_attrs: self.pending_attrs,
            children: self.children,
        }
    }
}

impl<'scope, T: Tag> AttributeBuilder<'scope> for TypedElement<'scope, T> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.pending_attrs
            .push(PendingAttribute::build(value.into_storable(), target));
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, E::EventType, M> + Clone + 'scope,
    {
        self.pending_attrs
            .push(PendingAttribute::new_scoped(move |element, owner| {
                bind_event(element, event, callback.clone(), owner)
            }));
        self
    }
}

impl<'scope, T: Tag> ApplyAttributes<'scope> for TypedElement<'scope, T> {
    fn apply_attributes(&mut self, attrs: Vec<PendingAttribute<'scope>>) {
        let mut current = std::mem::take(&mut self.pending_attrs);
        current.extend(attrs);
        self.pending_attrs = crate::attribute::consolidate_attributes(current);
    }
}

impl<'scope, T: Tag> View<'scope> for TypedElement<'scope, T> {
    fn mount(
        &self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) {
        let token = owner.token();
        let mut all_attrs = self.pending_attrs.clone();
        all_attrs.extend(attrs);
        let all_attrs = crate::attribute::consolidate_attributes(all_attrs);
        let mut inputs = RuntimeInputs::new();
        for attr in &all_attrs {
            inputs.extend(&attr.runtime_inputs());
        }
        if let Err(error) = token.validate_inputs(&inputs) {
            handle_error(error);
            return;
        }
        for attr in all_attrs {
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
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) where
        Self: Sized,
    {
        self.mount(owner, parent, attrs);
    }
}

impl<'scope, T: Tag> From<TypedElement<'scope, T>> for Element<'scope> {
    fn from(value: TypedElement<'scope, T>) -> Self {
        value.into_untyped()
    }
}

impl<'scope> std::ops::Deref for Element<'scope> {
    type Target = WebElem;

    fn deref(&self) -> &Self::Target {
        &self.dom_element
    }
}

impl<'scope, T: Tag> std::ops::Deref for TypedElement<'scope, T> {
    type Target = WebElem;

    fn deref(&self) -> &Self::Target {
        self.dom_element.as_ref()
    }
}

pub fn bind_event<'scope, E, F, M>(
    dom_element: &WebElem,
    event: E,
    callback: F,
    owner: &ViewOwnerToken<'scope>,
) where
    E: crate::event::EventDescriptor + 'static,
    F: EventHandler<'scope, E::EventType, M> + 'scope,
{
    let handler = callback.into_handler();
    bind_event_impl(dom_element, event.name().to_string(), handler, owner);
}

pub fn bind_event_impl<'scope, E>(
    dom_element: &WebElem,
    event_name: String,
    mut handler: Box<dyn FnMut(E) + 'scope>,
    owner: &ViewOwnerToken<'scope>,
) where
    E: FromWasmAbi + JsCast + 'static,
{
    if !owner.is_active() {
        return;
    }
    let destination = owner.host_callback(move |payload| {
        handler(payload.unchecked_into::<E>());
    });
    let destination_for_closure = destination.clone();
    let closure = Closure::wrap(Box::new(move |event: E| {
        let _ = destination_for_closure.dispatch(event.unchecked_into::<JsValue>());
    }) as Box<dyn FnMut(E)>);
    let closure = Rc::new(RefCell::new(Some(closure.into_js_value())));
    let js_fn = closure
        .borrow()
        .as_ref()
        .expect("element event callback is present")
        .unchecked_ref::<js_sys::Function>()
        .clone();
    if let Err(error) = dom_element
        .add_event_listener_with_callback(&event_name, &js_fn)
        .map_err(SilexError::from)
    {
        destination.cancel();
        let _ = closure.borrow_mut().take();
        handle_error(error);
        return;
    }

    let target = dom_element.clone();
    let event_name_for_cleanup = event_name.clone();
    let closure_for_cleanup = closure.clone();
    owner.host_resource_for_callback(&destination, move || {
        let _ = target.remove_event_listener_with_callback(&event_name_for_cleanup, &js_fn);
        let _ = closure_for_cleanup.borrow_mut().take();
    });
}
