use crate::attribute::{ApplyTarget, AttributeBuilder, IntoStorable, PendingAttribute};
use crate::event::{EventDescriptor, EventHandler};
use crate::view::{
    AnyView, ApplyAttributes, HostResourceHandle, OwnedViewOwner, View, ViewOwner, ViewOwnerToken,
};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue, convert::FromWasmAbi, prelude::*};
use web_sys::Element as WebElem;

use silex_core::{ReactiveError, RuntimeInputs, SilexError, SilexResult};

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
    ) -> SilexResult<()> {
        let provisional_scope = Rc::new(owner.try_owned_scope()?);
        let provisional_owner =
            OwnedViewOwner::new(provisional_scope.clone(), owner.token().error_handler());
        let token = provisional_owner.token();
        let mut appended = false;
        let result = (|| -> SilexResult<()> {
            let attrs = self.all_attrs(attrs);
            let mut inputs = RuntimeInputs::new();
            for attr in &attrs {
                inputs.extend(&attr.runtime_inputs());
            }
            token.validate_inputs(&inputs)?;
            for attr in attrs {
                attr.apply(&self.dom_element, &token)?;
            }
            parent.append_child(&self.dom_element)?;
            appended = true;
            for child in &self.children {
                child.mount(&provisional_owner, self.dom_element.as_ref(), Vec::new())?;
            }
            let scope_for_cleanup = provisional_scope.clone();
            owner.on_cleanup(
                Box::new(move || {
                    scope_for_cleanup.dispose();
                    Ok(())
                }),
                owner.token().error_handler(),
            )?;
            Ok(())
        })();

        if let Err(error) = result {
            rollback_mount(&provisional_scope, &self.dom_element, appended);
            return Err(error);
        }
        Ok(())
    }
}

fn rollback_mount<'scope>(
    scope: &Rc<silex_core::OwnedScope<'scope>>,
    element: &WebElem,
    appended: bool,
) {
    let dispose_panic = catch_unwind(AssertUnwindSafe(|| scope.dispose())).err();
    if appended && let Some(parent) = element.parent_node() {
        let _ = parent.remove_child(element);
    }
    if let Some(panic) = dispose_panic {
        resume_unwind(panic);
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
    ) -> SilexResult<()> {
        self.mount_inner(owner, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        self.mount_inner(owner, parent, attrs)
    }
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
    ) -> SilexResult<()> {
        let element = self.clone().into_untyped();
        element.mount_inner(owner, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &web_sys::Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> SilexResult<()>
    where
        Self: Sized,
    {
        let element = self.into_untyped();
        element.mount_inner(owner, parent, attrs)
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
) -> SilexResult<()>
where
    E: crate::event::EventDescriptor + 'static,
    F: EventHandler<'scope, E::EventType, M> + 'scope,
{
    let handler = callback.into_handler();
    bind_event_impl(dom_element, event.name().to_string(), handler, owner)
}

pub fn bind_event_impl<'scope, E>(
    dom_element: &WebElem,
    event_name: String,
    mut handler: Box<dyn FnMut(E) -> SilexResult<()> + 'scope>,
    owner: &ViewOwnerToken<'scope>,
) -> SilexResult<()>
where
    E: FromWasmAbi + JsCast + 'static,
{
    if !owner.is_active() {
        return Err(SilexError::Reactivity(ReactiveError::NoSuchNode));
    }
    let destination = owner.host_callback(
        move |payload| handler(payload.unchecked_into::<E>()),
        owner.error_handler(),
    );
    let destination_for_closure = std::panic::AssertUnwindSafe(destination.clone());
    let closure: Closure<dyn FnMut(E)> = Closure::wrap(Box::new(move |event: E| {
        let _ = destination_for_closure.dispatch(event.unchecked_into::<JsValue>());
    }));
    let resource = HostResourceHandle::from_js_callback(&destination, closure.into_js_value());
    let js_fn = resource.js_callback_function();
    if let Err(error) = dom_element
        .add_event_listener_with_callback(&event_name, &js_fn)
        .map_err(SilexError::from)
    {
        destination.cancel();
        resource.cancel_once();
        return Err(error);
    }

    let target = dom_element.clone();
    let event_name_for_cleanup = event_name.clone();
    let js_fn_for_cleanup = js_fn.clone();
    owner.try_host_resource_for_js_callback(&destination, resource, move || {
        let _ =
            target.remove_event_listener_with_callback(&event_name_for_cleanup, &js_fn_for_cleanup);
    })?;
    Ok(())
}
