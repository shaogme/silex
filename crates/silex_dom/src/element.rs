use crate::attribute::{ApplyTarget, AttrOp, AttributeBuilder, IntoStorable};
use crate::event::{EventDescriptor, EventHandler};
use crate::view::{
    AnyView, ApplyAttributes, HostResource, MountContext, MountInstance, MountOwner,
    MountOwnerToken, MountTarget, OwnerMount, View,
};
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use wasm_bindgen::{JsCast, JsValue, convert::FromWasmAbi, prelude::*};
use web_sys::Element as WebElem;

use silex_core::{CloseError, ErrorHandlerInput, ReactiveError, SilexError, SilexResult};

pub mod tags;
pub use tags::*;

pub fn text<'scope, V: View<'scope>>(content: V) -> V {
    content
}

pub struct Element<'scope> {
    tag_name: String,
    namespace: Option<String>,
    pub(crate) pending_attrs: Vec<AttrOp<'scope>>,
    pub(crate) children: Vec<AnyView<'scope>>,
}

impl<'scope> Clone for Element<'scope> {
    fn clone(&self) -> Self {
        Self {
            tag_name: self.tag_name.clone(),
            namespace: self.namespace.clone(),
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
        }
    }
}

impl PartialEq for Element<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.tag_name == other.tag_name
            && self.namespace == other.namespace
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope> Element<'scope> {
    pub fn new(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: None,
            pending_attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn new_svg(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: Some("http://www.w3.org/2000/svg".to_string()),
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

    fn all_attrs(&self, attrs: Vec<AttrOp<'scope>>) -> Vec<AttrOp<'scope>> {
        let mut all = self.pending_attrs.clone();
        all.extend(attrs);
        crate::attribute::consolidate_attributes(all)
    }

    fn mount_inner(
        &self,
        context: &MountContext<'scope>,
        attrs: Vec<AttrOp<'scope>>,
    ) -> SilexResult<MountInstance<'scope>> {
        let document = crate::document();
        let dom_element = match self.namespace.as_deref() {
            Some(namespace) => document
                .create_element_ns(Some(namespace), &self.tag_name)
                .map_err(SilexError::fatal)?,
            None => document
                .create_element(&self.tag_name)
                .map_err(SilexError::fatal)?,
        };
        let owner = context.owner();
        let provisional_owner = OwnerMount::new(owner.child());
        let token = provisional_owner.token();
        let mut appended = false;
        let result = (|| -> SilexResult<MountInstance<'scope>> {
            let child_context = context.with_parts(
                MountTarget::Append(dom_element.clone().into()),
                context.ancestry().push(&dom_element),
                token.clone(),
                context.transaction().clone(),
            );
            context.target().append(dom_element.as_ref())?;
            appended = true;
            let attrs = self.all_attrs(attrs);
            for attr in attrs {
                attr.apply(&dom_element, &child_context)?;
            }
            for child in &self.children {
                let _ = child.mount(&child_context, Vec::new())?;
            }
            let owner_for_cleanup = provisional_owner.token();
            let element_for_cleanup = dom_element.clone();
            owner.on_cleanup(
                Box::new(move || {
                    owner_for_cleanup.close().map_err(|error| {
                        SilexError::fatal(silex_core::SilexErrorKind::Close(error))
                    })?;
                    if let Some(parent) = element_for_cleanup.parent_node() {
                        let _ = parent.remove_child(&element_for_cleanup);
                    }
                    Ok(())
                }),
                context.error_handler(),
            )?;
            Ok(MountInstance::from_nodes(vec![dom_element.clone().into()]))
        })();

        if let Err(error) = &result {
            rollback_mount(&provisional_owner, &dom_element, appended);
            return Err(error.clone());
        }
        result
    }
}

fn rollback_mount<'scope>(owner: &OwnerMount<'scope>, element: &WebElem, appended: bool) {
    let close_panic = match catch_unwind(AssertUnwindSafe(|| owner.token().close())) {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(panic) => Some(CloseError::from_panic(panic)),
    };
    if appended && let Some(parent) = element.parent_node() {
        let _ = parent.remove_child(element);
    }
    if let Some(error) = close_panic {
        owner.token().report_close_error(error);
    }
}

impl<'scope> AttributeBuilder<'scope> for Element<'scope> {
    fn build_attribute<V>(mut self, target: ApplyTarget, value: V) -> Self
    where
        V: IntoStorable<'scope>,
    {
        self.pending_attrs
            .push(AttrOp::build(value.into_storable(), target));
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, E::EventType, M> + Clone + 'scope,
    {
        self.pending_attrs
            .push(AttrOp::new_scoped(move |element, context| {
                let owner = context.owner();
                bind_event(
                    element,
                    event,
                    callback.clone(),
                    &owner,
                    context.error_handler(),
                )
            }));
        self
    }
}

impl<'scope> ApplyAttributes<'scope> for Element<'scope> {
    fn apply_attributes(&mut self, attrs: Vec<AttrOp<'scope>>) {
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
        context: &MountContext<'scope>,
        attrs: Vec<AttrOp<'scope>>,
    ) -> SilexResult<MountInstance<'scope>> {
        self.mount_inner(context, attrs)
    }
}

pub struct TypedElement<'scope, T: Tag> {
    tag_name: String,
    namespace: Option<String>,
    pub(crate) pending_attrs: Vec<AttrOp<'scope>>,
    pub(crate) children: Vec<AnyView<'scope>>,
    marker: PhantomData<T>,
}

impl<'scope, T: Tag> Clone for TypedElement<'scope, T> {
    fn clone(&self) -> Self {
        Self {
            tag_name: self.tag_name.clone(),
            namespace: self.namespace.clone(),
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
            marker: PhantomData,
        }
    }
}

impl<T: Tag> PartialEq for TypedElement<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.tag_name == other.tag_name
            && self.namespace == other.namespace
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope, T: Tag> TypedElement<'scope, T> {
    pub fn new(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: None,
            pending_attrs: Vec::new(),
            children: Vec::new(),
            marker: PhantomData,
        }
    }

    pub fn new_svg(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: Some("http://www.w3.org/2000/svg".to_string()),
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

    #[doc(hidden)]
    pub fn with_child_using<V>(tag: &str, child: V, constructor: fn(&str) -> Self) -> Self
    where
        V: View<'scope> + 'scope,
    {
        let mut element = constructor(tag);
        element.children.push(child.into_any());
        element
    }

    pub fn into_untyped(self) -> Element<'scope> {
        Element {
            tag_name: self.tag_name,
            namespace: self.namespace,
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
            .push(AttrOp::build(value.into_storable(), target));
        self
    }

    fn build_event<E, F, M>(mut self, event: E, callback: F) -> Self
    where
        E: EventDescriptor + 'static,
        F: EventHandler<'scope, E::EventType, M> + Clone + 'scope,
    {
        self.pending_attrs
            .push(AttrOp::new_scoped(move |element, context| {
                let owner = context.owner();
                bind_event(
                    element,
                    event,
                    callback.clone(),
                    &owner,
                    context.error_handler(),
                )
            }));
        self
    }
}

impl<'scope, T: Tag> ApplyAttributes<'scope> for TypedElement<'scope, T> {
    fn apply_attributes(&mut self, attrs: Vec<AttrOp<'scope>>) {
        let mut current = std::mem::take(&mut self.pending_attrs);
        current.extend(attrs);
        self.pending_attrs = crate::attribute::consolidate_attributes(current);
    }
}

impl<'scope, T: Tag> View<'scope> for TypedElement<'scope, T> {
    fn mount(
        &self,
        context: &MountContext<'scope>,
        attrs: Vec<AttrOp<'scope>>,
    ) -> SilexResult<MountInstance<'scope>> {
        let element = self.clone().into_untyped();
        element.mount_inner(context, attrs)
    }
}

impl<'scope, T: Tag> From<TypedElement<'scope, T>> for Element<'scope> {
    fn from(value: TypedElement<'scope, T>) -> Self {
        value.into_untyped()
    }
}

pub fn bind_event<'scope, E, F, M, H>(
    dom_element: &WebElem,
    event: E,
    callback: F,
    owner: &MountOwnerToken<'scope>,
    error_handler: H,
) -> SilexResult<()>
where
    E: crate::event::EventDescriptor + 'static,
    F: EventHandler<'scope, E::EventType, M> + 'scope,
    H: ErrorHandlerInput<'scope>,
{
    let handler = callback.into_handler();
    bind_event_impl(
        dom_element,
        event.name().to_string(),
        handler,
        owner,
        error_handler.handler_ref(),
    )
}

pub fn bind_event_impl<'scope, E, H>(
    dom_element: &WebElem,
    event_name: String,
    mut handler: Box<dyn FnMut(E) -> SilexResult<()> + 'scope>,
    owner: &MountOwnerToken<'scope>,
    error_handler: H,
) -> SilexResult<()>
where
    E: FromWasmAbi + JsCast + 'static,
    H: ErrorHandlerInput<'scope>,
{
    let error_handler = error_handler.handler_ref();
    if !owner.is_active()? {
        return Err(SilexError::fatal(ReactiveError::NoSuchNode));
    }
    let destination = owner.host_callback(
        move |payload| handler(payload.unchecked_into::<E>()),
        error_handler,
    )?;
    let destination_for_closure = std::panic::AssertUnwindSafe(destination.clone());
    let closure: Closure<dyn FnMut(E)> = Closure::wrap(Box::new(move |event: E| {
        let _ = destination_for_closure.dispatch(event.unchecked_into::<JsValue>());
    }));
    let resource = HostResource::from_js_callback(&destination, closure.into_js_value());
    let js_fn = resource.js_callback_function();
    if let Err(error) = dom_element
        .add_event_listener_with_callback(&event_name, &js_fn)
        .map_err(SilexError::fatal)
    {
        let _ = destination.cancel();
        let _ = resource.cancel_once();
        return Err(error);
    }

    let target = dom_element.clone();
    let event_name_for_cleanup = event_name.clone();
    let js_fn_for_cleanup = js_fn.clone();
    owner.host_resource_for_js_callback(
        &destination,
        resource,
        move || {
            let _ = target
                .remove_event_listener_with_callback(&event_name_for_cleanup, &js_fn_for_cleanup);
        },
        error_handler,
    )?;
    Ok(())
}
