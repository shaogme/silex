use silex_core::{EffectPhase, RxReadRef, SilexError, SilexErrorKind, SilexResult, reactivity::Rx};
use silex_dom::{
    DomContext, DomElement, DomNode,
    attribute::{AttributeRequest, AttributeTarget, AttributeValue},
    tree::ElementSpec,
};
use silex_macros::component;
use silex_view::{
    AnyView, ApplyTarget, AttrOp, IntoStorable, MountContext, MountInstance, MountOwner,
    MountOwnerToken, MountTarget, MountTransaction, View, ViewError,
};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

const PORTAL_VISIBILITY_ROOT_ATTRIBUTE: &str = "data-portal-visibility-root";

#[derive(Clone)]
pub(crate) struct PortalVisibilityRoot {
    dom: DomContext,
    element: DomElement,
}

impl PortalVisibilityRoot {
    pub(crate) fn create(dom: DomContext, open: bool) -> SilexResult<Self> {
        let element = dom.create_element(ElementSpec::new("div"))?;
        set_attribute(
            &dom,
            &element,
            PORTAL_VISIBILITY_ROOT_ATTRIBUTE,
            AttributeValue::Empty,
        )?;
        let root = Self { dom, element };
        root.set_open(open)?;
        Ok(root)
    }

    pub(crate) fn element(&self) -> &DomElement {
        &self.element
    }

    pub(crate) fn set_open(&self, open: bool) -> SilexResult<()> {
        set_attribute(
            &self.dom,
            &self.element,
            "data-state",
            AttributeValue::text(if open { "open" } else { "closed" }),
        )?;
        set_attribute(
            &self.dom,
            &self.element,
            "aria-hidden",
            AttributeValue::text(if open { "false" } else { "true" }),
        )?;
        set_attribute(
            &self.dom,
            &self.element,
            "style",
            AttributeValue::text(if open {
                "display: contents !important; pointer-events: auto;"
            } else {
                "display: none !important; pointer-events: none;"
            }),
        )
    }
}

#[derive(Clone, Default)]
pub struct PortalHostAttrs<'scope> {
    attrs: Vec<AttrOp<'scope>>,
}

impl<'scope> PortalHostAttrs<'scope> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attr<V>(self, name: impl Into<Cow<'static, str>>, value: V) -> SilexResult<Self>
    where
        V: IntoStorable<'scope>,
    {
        self.try_attr(name, value)
    }

    pub fn try_attr<V>(mut self, name: impl Into<Cow<'static, str>>, value: V) -> SilexResult<Self>
    where
        V: IntoStorable<'scope>,
    {
        let target = ApplyTarget::attr(name);
        if let Some(name) = reserved_host_attribute(&target) {
            return Err(SilexError::from(ViewError::Invariant {
                operation: "PortalHostAttrs::try_attr",
                message: format!("field {name} is reserved; use the Portal open signal"),
            }));
        }
        self.attrs
            .push(AttrOp::build(value.into_storable(), target));
        Ok(self)
    }

    pub fn class<V>(self, value: V) -> SilexResult<Self>
    where
        V: IntoStorable<'scope>,
    {
        self.attr("class", value)
    }

    pub fn id<V>(self, value: V) -> SilexResult<Self>
    where
        V: IntoStorable<'scope>,
    {
        self.attr("id", value)
    }

    pub fn title<V>(self, value: V) -> SilexResult<Self>
    where
        V: IntoStorable<'scope>,
    {
        self.attr("title", value)
    }

    pub fn data<V>(self, name: impl Into<String>, value: V) -> SilexResult<Self>
    where
        V: IntoStorable<'scope>,
    {
        self.attr(Cow::Owned(format!("data-{}", name.into())), value)
    }

    fn into_attrs(self) -> Vec<AttrOp<'scope>> {
        self.attrs
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortalContentMode {
    #[default]
    KeepAlive,
    UnmountWhenClosed,
}

#[derive(Clone)]
struct PortalView<'scope> {
    children: AnyView<'scope>,
    open: Option<Rx<'scope, bool>>,
    content_mode: PortalContentMode,
    mount_to: Option<DomNode>,
    host_attrs: PortalHostAttrs<'scope>,
}

fn set_attribute(
    dom: &DomContext,
    element: &DomElement,
    name: &str,
    value: AttributeValue,
) -> SilexResult<()> {
    dom.set_attribute(AttributeRequest::new(
        element,
        AttributeTarget::named(name),
        value,
    ))?;
    Ok(())
}

fn close_owner<'scope>(owner: &MountOwnerToken<'scope>) -> SilexResult<()> {
    owner
        .close()
        .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
}

fn remove_node(dom: &DomContext, node: &DomNode) -> SilexResult<()> {
    if dom.parent(node)?.is_some() {
        dom.remove(node)?;
    }
    Ok(())
}

fn cleanup_failed_mount<'scope>(dom: &DomContext, owner: &MountOwnerToken<'scope>, host: &DomNode) {
    let _ = close_owner(owner);
    let _ = remove_node(dom, host);
}

fn reserved_host_attribute(target: &ApplyTarget) -> Option<&'static str> {
    match target.attr_name() {
        "hidden" => Some("hidden"),
        "aria-hidden" => Some("aria-hidden"),
        "inert" => Some("inert"),
        "data-state" => Some("data-state"),
        "style" => Some("style"),
        _ => None,
    }
}

impl<'scope> PortalView<'scope> {
    fn mount_inner(self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let attrs = self.host_attrs.into_attrs();
        let dom = context.dom().clone();
        let target = match self.mount_to.clone() {
            Some(target) => target,
            None => dom
                .document_body()?
                .ok_or_else(|| {
                    SilexError::from(ViewError::Invariant {
                        operation: "Portal::mount",
                        message: "document.body is required when no target is supplied".to_string(),
                    })
                })?
                .node()
                .clone(),
        };
        // Do not subscribe the surrounding render effect to Portal visibility.
        let initial_open = self
            .open
            .as_ref()
            .map_or(Ok(true), |open| open.with_untracked(|value| *value))?;
        let host = dom.create_element(ElementSpec::new("div"))?;
        let root = PortalVisibilityRoot::create(dom.clone(), initial_open)?;
        dom.append(host.node(), root.element().node())?;
        let host_owner = context.owner().child();
        let attached = Rc::new(Cell::new(false));
        let error_handler = context.error_handler();
        let host_context = context.with_parts(
            MountTarget::append(dom.clone(), host.node().clone()),
            context.ancestry().push(&host),
            host_owner.clone(),
            context.transaction().clone(),
        );
        for attr in attrs {
            attr.apply(&host, &host_context)?;
        }

        let host_for_commit = host.node().clone();
        let target_for_commit = target.clone();
        let dom_for_commit = dom.clone();
        let attached_for_commit = attached.clone();
        let children = self.children;
        let open = self.open;
        let content_mode = self.content_mode;
        let root_for_content = root.clone();
        let context_for_content = context.clone();
        let host_for_content = host.clone();
        let host_owner_for_content = host_owner.clone();
        context.on_commit(move || {
            let attempt = catch_unwind(AssertUnwindSafe(|| {
                (|| {
                    dom_for_commit.append(&target_for_commit, &host_for_commit)?;
                    attached_for_commit.set(true);
                    match content_mode {
                        PortalContentMode::KeepAlive => {
                            let child_owner = host_owner_for_content.child();
                            let transaction = MountTransaction::new();
                            let child_context = context_for_content.with_parts(
                                MountTarget::append(
                                    dom_for_commit.clone(),
                                    root_for_content.element().node().clone(),
                                ),
                                context_for_content.ancestry().push(&host_for_content),
                                child_owner.clone(),
                                transaction.clone(),
                            );
                            let result = child_context
                                .mount(&children)
                                .and_then(|_| transaction.commit());
                            if result.is_err() {
                                let _ = transaction.rollback();
                                let _ = close_owner(&child_owner);
                            }
                            result
                        }
                        PortalContentMode::UnmountWhenClosed => {
                            let open = open.ok_or_else(|| {
                                SilexError::from(ViewError::Invariant {
                                    operation: "Portal::mount",
                                    message: "UnmountWhenClosed requires an open signal"
                                        .to_string(),
                                })
                            })?;
                            let active = Rc::new(RefCell::new(
                                None::<(MountOwnerToken<'scope>, MountInstance<'scope>)>,
                            ));
                            let dom_for_effect = dom_for_commit.clone();
                            let context_for_effect = context_for_content.clone();
                            let root_for_effect = root_for_content.element().node().clone();
                            let parent_owner = host_owner_for_content.clone();
                            let children_for_effect = children.clone();
                            let host_for_effect = host_for_content.clone();
                            host_owner_for_content.effect(
                                EffectPhase::Normal,
                                Box::new(move || {
                                    if !attached.get() {
                                        return Ok(());
                                    }
                                    if open.with(|value| *value)? {
                                        if active.borrow().is_none() {
                                            let child_owner = parent_owner.child();
                                            let transaction = MountTransaction::new();
                                            let child_context = context_for_effect.with_parts(
                                                MountTarget::append(
                                                    dom_for_effect.clone(),
                                                    root_for_effect.clone(),
                                                ),
                                                context_for_effect
                                                    .ancestry()
                                                    .push(&host_for_effect),
                                                child_owner.clone(),
                                                transaction.clone(),
                                            );
                                            let children = children_for_effect.clone();
                                            let instance = child_context.mount(&children)?;
                                            transaction.commit()?;
                                            *active.borrow_mut() = Some((child_owner, instance));
                                        }
                                    } else if let Some((child_owner, instance)) =
                                        active.borrow_mut().take()
                                    {
                                        close_owner(&child_owner)?;
                                        for node in instance.nodes() {
                                            remove_node(&dom_for_effect, node)?;
                                        }
                                    }
                                    Ok(())
                                }),
                                error_handler,
                            )
                        }
                    }
                })()
            }));
            match attempt {
                Ok(result) => {
                    if result.is_err() {
                        cleanup_failed_mount(
                            &dom_for_commit,
                            &host_owner_for_content,
                            &host_for_commit,
                        );
                    }
                    result
                }
                Err(panic) => {
                    cleanup_failed_mount(
                        &dom_for_commit,
                        &host_owner_for_content,
                        &host_for_commit,
                    );
                    resume_unwind(panic);
                }
            }
        })?;

        if let Some(open) = open {
            let root_for_effect = root.clone();
            let effect_owner = host_owner.clone();
            context.on_commit(move || {
                effect_owner.effect(
                    EffectPhase::Normal,
                    Box::new(move || root_for_effect.set_open(open.with(|value| *value)?)),
                    error_handler,
                )
            })?;
        }

        let dom_for_cleanup = dom.clone();
        let host_node = host.node().clone();
        let host_node_for_cleanup = host_node.clone();
        context.owner().on_cleanup(
            Box::new(move || {
                close_owner(&host_owner)?;
                remove_node(&dom_for_cleanup, &host_node_for_cleanup)
            }),
            error_handler,
        )?;
        Ok(MountInstance::from_nodes(vec![host_node]))
    }
}

impl<'scope> View<'scope> for PortalView<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        self.clone().mount_inner(context)
    }
}

#[component]
pub fn PortalHost<'scope, Ctx>(
    #[ctx] _ctx: Ctx,
    #[prop(render)]
    #[chain]
    children: AnyView<'scope>,
    #[chain(default)] host_attrs: PortalHostAttrs<'scope>,
    #[chain(default)] mount_to: Option<DomNode>,
) -> impl View<'scope> {
    PortalView {
        children,
        open: None,
        content_mode: PortalContentMode::KeepAlive,
        mount_to,
        host_attrs,
    }
}

#[component]
pub fn Portal<'scope, Ctx>(
    #[ctx] _ctx: Ctx,
    #[prop(into)] open: Rx<'scope, bool>,
    #[prop(render)]
    #[chain]
    children: AnyView<'scope>,
    #[chain(default)] host_attrs: PortalHostAttrs<'scope>,
    #[chain(default)] content_mode: PortalContentMode,
    #[chain(default)] mount_to: Option<DomNode>,
) -> impl View<'scope> {
    PortalView {
        children,
        open: Some(open),
        content_mode,
        mount_to,
        host_attrs,
    }
}
