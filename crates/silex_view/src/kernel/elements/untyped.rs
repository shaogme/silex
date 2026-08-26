use super::erased::AnyView;
use crate::events::{EventDescriptor, EventHandler, bind_event};
use crate::kernel::attributes::{
    ApplyTarget, AttrOp, AttributeBuilder, IntoStorable, consolidate_attributes,
};
use crate::kernel::{MountContext, MountInstance, MountTarget, View};
use crate::lifecycle::MountOwnerToken;
use silex_core::{CloseError, SilexError, SilexErrorKind, SilexResult};
use silex_dom::model::{DomNode, ElementSpec, Namespace};
use std::panic::{AssertUnwindSafe, catch_unwind};
pub struct Element<'scope> {
    pub(super) tag_name: String,
    pub(super) namespace: Option<Namespace>,
    pub(super) is_void: bool,
    pub(crate) pending_attrs: Vec<AttrOp<'scope>>,
    pub(crate) children: Vec<AnyView<'scope>>,
}

impl<'scope> Clone for Element<'scope> {
    fn clone(&self) -> Self {
        Self {
            tag_name: self.tag_name.clone(),
            namespace: self.namespace.clone(),
            is_void: self.is_void,
            pending_attrs: self.pending_attrs.clone(),
            children: self.children.clone(),
        }
    }
}

impl PartialEq for Element<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.tag_name == other.tag_name
            && self.namespace == other.namespace
            && self.is_void == other.is_void
            && self.pending_attrs == other.pending_attrs
            && self.children == other.children
    }
}

impl<'scope> Element<'scope> {
    pub fn new(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: None,
            is_void: false,
            pending_attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn new_svg(tag: &str) -> Self {
        Self {
            tag_name: tag.to_string(),
            namespace: Some(Namespace::Svg),
            is_void: false,
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

    fn spec(&self) -> ElementSpec {
        self.namespace.clone().map_or_else(
            || {
                if self.is_void {
                    ElementSpec::namespaced(self.tag_name.clone(), Namespace::Html, true)
                } else {
                    ElementSpec::new(self.tag_name.clone())
                }
            },
            |namespace| ElementSpec::namespaced(self.tag_name.clone(), namespace, self.is_void),
        )
    }

    fn mount_inner(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let dom_element = context.dom().create_element(self.spec())?;
        let parent_owner = context.owner();
        let provisional_owner = parent_owner.child();
        let token = provisional_owner.clone();
        let cleanup_dom = context.dom().clone();
        let cleanup_handler = context.error_handler();
        let mut appended = false;
        let result = (|| -> SilexResult<MountInstance<'scope>> {
            let child_context = context.with_parts(
                MountTarget::append(context.dom().clone(), dom_element.node().clone()),
                context.ancestry().push(&dom_element),
                token.clone(),
                context.transaction().clone(),
            );
            context.target().append_node(dom_element.node())?;
            appended = true;
            for attr in consolidate_attributes(self.pending_attrs.clone()) {
                attr.apply(&dom_element, &child_context)?;
            }
            for child in &self.children {
                let _ = child_context.mount(child)?;
            }
            let owner_for_cleanup = provisional_owner.clone();
            let element_for_cleanup = dom_element.node().clone();
            parent_owner.on_cleanup(
                Box::new(move || {
                    owner_for_cleanup
                        .close()
                        .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))?;
                    if cleanup_dom.parent(&element_for_cleanup)?.is_some() {
                        cleanup_dom.remove(&element_for_cleanup)?;
                    }
                    Ok(())
                }),
                cleanup_handler,
            )?;
            Ok(MountInstance::from_nodes(vec![dom_element.node().clone()]))
        })();
        if let Err(error) = &result {
            rollback_mount(context, &provisional_owner, dom_element.node(), appended);
            return Err(error.clone());
        }
        result
    }
}

fn rollback_mount<'scope>(
    context: &MountContext<'scope>,
    owner: &MountOwnerToken<'scope>,
    element: &DomNode,
    appended: bool,
) {
    let close_result = catch_unwind(AssertUnwindSafe(|| owner.close()));
    if appended && context.dom().parent(element).ok().flatten().is_some() {
        let _ = context.dom().remove(element);
    }
    match close_result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => owner.report_close_error(error),
        Err(panic) => owner.report_close_error(CloseError::from_panic(panic)),
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
        F: EventHandler<'scope, M> + Clone + 'scope,
    {
        self.pending_attrs
            .push(AttrOp::new_scoped(move |element, context| {
                bind_event(
                    context,
                    element,
                    event,
                    callback.clone(),
                    context.error_handler(),
                )
            }));
        self
    }
}

impl<'scope> View<'scope> for Element<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        self.mount_inner(context)
    }
}
