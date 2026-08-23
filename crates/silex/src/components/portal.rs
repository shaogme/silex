use silex_core::{EffectPhase, RxReadRef, SilexError, SilexErrorKind, SilexResult, reactivity::Rx};
use silex_dom::view::{
    MountContext, MountErrorHandler, MountOwner, MountOwnerToken, MountTarget, MountTransaction,
};
use silex_dom::{document, prelude::*};
use silex_macros::component;
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};

const PORTAL_VISIBILITY_ROOT_ATTRIBUTE: &str = "data-portal-visibility-root";

/// Framework-owned DOM boundary that controls Portal visibility.
///
/// The root is intentionally created separately from the public host. No
/// caller-provided attributes are ever applied to this element, so its
/// visibility state cannot be replaced by Portal host attributes.
#[derive(Clone)]
pub(crate) struct PortalVisibilityRoot {
    element: HtmlElement,
}

impl PortalVisibilityRoot {
    pub(crate) fn create(open: bool) -> SilexResult<Self> {
        let element = document()
            .create_element("div")
            .map_err(SilexError::fatal)?
            .dyn_into::<HtmlElement>()
            .map_err(|_| {
                SilexError::fatal(SilexErrorKind::Dom(
                    "Portal visibility root must be an HTML element".to_string(),
                ))
            })?;
        element
            .set_attribute(PORTAL_VISIBILITY_ROOT_ATTRIBUTE, "")
            .map_err(SilexError::fatal)?;
        let root = Self { element };
        root.set_open(open)?;
        Ok(root)
    }

    pub(crate) fn element(&self) -> &HtmlElement {
        &self.element
    }

    pub(crate) fn set_open(&self, open: bool) -> SilexResult<()> {
        update_visibility_state(&self.element, open)
    }
}

/// Explicit attribute entry point for a Portal host.
///
/// Visibility state belongs to the private root. This builder therefore
/// rejects framework-owned fields while retaining ordinary host diagnostics,
/// identity and event attributes for callers.
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
            return Err(SilexError::fatal(SilexErrorKind::Dom(format!(
                "PortalHostAttrs field `{name}` is reserved; use the Portal open signal"
            ))));
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

/// Controls whether a Portal keeps its content owner while it is hidden.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortalContentMode {
    /// Keep the content DOM, event handlers and child resources alive.
    #[default]
    KeepAlive,
    /// Keep the host and slot, but unmount the content while closed.
    UnmountWhenClosed,
}

#[derive(Clone)]
struct PortalView<'scope> {
    children: AnyView<'scope>,
    open: Option<Rx<'scope, bool>>,
    content_mode: PortalContentMode,
    mount_to: Option<Node>,
    host_attrs: PortalHostAttrs<'scope>,
}

fn close_owner<'scope>(owner: &MountOwnerToken<'scope>) -> SilexResult<()> {
    owner
        .close()
        .map_err(|error| SilexError::fatal(SilexErrorKind::Close(error)))
}

fn remove_node(node: &Node) -> SilexResult<()> {
    if let Some(parent) = node.parent_node() {
        parent.remove_child(node).map_err(SilexError::fatal)?;
    }
    Ok(())
}

fn cleanup_failed_portal_mount<'scope>(
    host_owner: &MountOwnerToken<'scope>,
    content_owner: &MountOwnerToken<'scope>,
    host: &Node,
    error_handler: MountErrorHandler<'scope>,
) {
    for result in [
        close_owner(content_owner),
        close_owner(host_owner),
        remove_node(host),
    ] {
        if let Err(error) = result {
            let _ = error_handler.handle(error);
        }
    }
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

fn reserved_attribute_in_op<'scope>(op: &AttrOp<'scope>) -> Option<&'static str> {
    match op {
        AttrOp::Update(update) => reserved_host_attribute(&update.target),
        AttrOp::CombinedStyles(_) => Some("style"),
        AttrOp::Reactive(plan) => match &plan.target {
            ReactiveBindingTarget::Attribute(target) => reserved_host_attribute(target),
            ReactiveBindingTarget::StyleProperty(_) | ReactiveBindingTarget::DynamicStyle => {
                Some("style")
            }
            _ => None,
        },
        AttrOp::Sequence(ops) => ops.iter().find_map(reserved_attribute_in_op),
        AttrOp::CombinedClasses(_) | AttrOp::Custom { .. } | AttrOp::Noop => None,
    }
}

fn validate_host_attributes<'scope>(attrs: &[AttrOp<'scope>]) -> SilexResult<()> {
    if let Some(name) = attrs.iter().find_map(reserved_attribute_in_op) {
        return Err(SilexError::fatal(SilexErrorKind::Dom(format!(
            "Portal host attribute `{name}` is reserved; use the Portal open signal or host API"
        ))));
    }
    Ok(())
}

fn update_visibility_state(element: &HtmlElement, open: bool) -> SilexResult<()> {
    let element_ref: &Element = element.as_ref();
    element_ref
        .set_attribute("data-state", if open { "open" } else { "closed" })
        .map_err(SilexError::fatal)?;
    element_ref
        .set_attribute("aria-hidden", if open { "false" } else { "true" })
        .map_err(SilexError::fatal)?;
    element
        .style()
        .set_property_with_priority(
            "display",
            if open { "contents" } else { "none" },
            "important",
        )
        .map_err(SilexError::fatal)?;
    element
        .style()
        .set_property("pointer-events", if open { "auto" } else { "none" })
        .map_err(SilexError::fatal)
}

impl<'scope> PortalView<'scope> {
    fn mount_inner(self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        let host_attrs = self.host_attrs.into_attrs();
        validate_host_attributes(&host_attrs)?;
        let owner = context.owner();
        let error_handler = context.error_handler();
        let document = document();
        let target = match self.mount_to {
            Some(target) => target,
            None => document
                .body()
                .ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Dom(
                        "Portal requires document.body when no target is supplied".to_string(),
                    ))
                })?
                .into(),
        };
        let initial_open = match self.open {
            Some(open) => open.with(|value| *value)?,
            None => true,
        };
        let host_element = document.create_element("div").map_err(SilexError::fatal)?;
        let host: Node = host_element.clone().into();
        let visibility_root = PortalVisibilityRoot::create(initial_open)?;
        let root_element = visibility_root.element().clone();
        host_element
            .append_child(root_element.as_ref())
            .map_err(SilexError::fatal)?;
        let host_owner = owner.child();
        let host_token = host_owner.clone();
        let attached = Rc::new(Cell::new(false));

        let setup_result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
            let attached_for_commit = attached.clone();
            let host_for_commit = host.clone();
            context.on_commit(move || {
                MountTarget::Append(target.clone()).append(&host_for_commit)?;
                attached_for_commit.set(true);
                Ok(())
            })?;

            let host_context = context.with_parts(
                MountTarget::Append(host.clone()),
                context.ancestry().push(&host_element),
                host_token.clone(),
                context.transaction().clone(),
            );
            for attr in host_attrs {
                attr.apply(&host_element, &host_context)?;
            }
            let host_html = host_element
                .clone()
                .dyn_into::<HtmlElement>()
                .map_err(|_| {
                    SilexError::fatal(SilexErrorKind::Dom(
                        "Portal host must be an HTML element".to_string(),
                    ))
                })?;
            host_html
                .style()
                .set_property("display", "contents")
                .map_err(SilexError::fatal)?;
            if let Some(open) = self.open {
                let root_for_effect = PortalVisibilityRoot {
                    element: root_element.clone(),
                };
                let effect_owner = host_owner.clone();
                context.on_commit(move || {
                    effect_owner.effect(
                        EffectPhase::Normal,
                        Box::new(move || {
                            let visible = open.with(|value| *value)?;
                            root_for_effect.set_open(visible)
                        }),
                        error_handler,
                    )
                })?;
            }

            match self.content_mode {
                PortalContentMode::KeepAlive => {
                    let content_owner = host_owner.child();
                    let children = self.children.clone();
                    let portal_context = context.clone();
                    let content_root: Node = root_element.clone().into();
                    let failed_mount_host = host.clone();
                    let failed_mount_owner = host_owner.clone();
                    let attached_for_content = attached.clone();
                    context.on_commit(move || {
                        if !attached_for_content.get() {
                            cleanup_failed_portal_mount(
                                &failed_mount_owner,
                                &content_owner,
                                &failed_mount_host,
                                error_handler,
                            );
                            return Err(SilexError::fatal(SilexErrorKind::Dom(
                                "Portal host target was not committed".to_string(),
                            )));
                        }
                        let content_transaction = MountTransaction::new();
                        let content_context = portal_context.with_parts(
                            MountTarget::Append(content_root.clone()),
                            portal_context.ancestry().clone(),
                            content_owner.clone(),
                            content_transaction.clone(),
                        );
                        let result =
                            catch_unwind(AssertUnwindSafe(|| children.mount(&content_context)));
                        match result {
                            Ok(Ok(_instance)) => match content_transaction.commit() {
                                Ok(()) => Ok(()),
                                Err(error) => {
                                    cleanup_failed_portal_mount(
                                        &failed_mount_owner,
                                        &content_owner,
                                        &failed_mount_host,
                                        error_handler,
                                    );
                                    Err(error)
                                }
                            },
                            Ok(Err(error)) => {
                                let _ = content_transaction.rollback();
                                cleanup_failed_portal_mount(
                                    &failed_mount_owner,
                                    &content_owner,
                                    &failed_mount_host,
                                    error_handler,
                                );
                                Err(error)
                            }
                            Err(panic) => {
                                let _ = content_transaction.rollback();
                                cleanup_failed_portal_mount(
                                    &failed_mount_owner,
                                    &content_owner,
                                    &failed_mount_host,
                                    error_handler,
                                );
                                resume_unwind(panic);
                            }
                        }
                    })?;
                }
                PortalContentMode::UnmountWhenClosed => {
                    let slot_element = document.create_element("div").map_err(SilexError::fatal)?;
                    slot_element
                        .set_attribute("data-portal-content-slot", "")
                        .map_err(SilexError::fatal)?;
                    let slot_html =
                        slot_element
                            .clone()
                            .dyn_into::<HtmlElement>()
                            .map_err(|_| {
                                SilexError::fatal(SilexErrorKind::Dom(
                                    "Portal content slot must be an HTML element".to_string(),
                                ))
                            })?;
                    slot_html
                        .style()
                        .set_property("display", "contents")
                        .map_err(SilexError::fatal)?;
                    let slot: Node = slot_element.clone().into();
                    root_element
                        .append_child(&slot)
                        .map_err(SilexError::fatal)?;

                    let slot_for_cleanup = slot.clone();
                    host_owner.on_cleanup(
                        Box::new(move || remove_node(&slot_for_cleanup)),
                        error_handler,
                    )?;

                    let active_content = Rc::new(RefCell::new(None));
                    let active_content_for_effect = active_content.clone();
                    let children = self.children.clone();
                    let parent_owner = host_owner.clone();
                    let open = self.open;
                    let portal_context = context.clone();
                    let effect_owner = host_owner.clone();
                    let attached_for_effect = attached.clone();
                    context.on_commit(move || {
                        effect_owner.effect(
                            EffectPhase::Normal,
                            Box::new(move || -> SilexResult<()> {
                                if !attached_for_effect.get() {
                                    return Err(SilexError::fatal(SilexErrorKind::Dom(
                                        "Portal host target was not committed".to_string(),
                                    )));
                                }
                                let visible = open
                                    .ok_or_else(|| {
                                        SilexError::fatal(SilexErrorKind::Dom(
                                            "UnmountWhenClosed Portal requires an open signal"
                                                .to_string(),
                                        ))
                                    })?
                                    .with(|value| *value)?;
                                if visible {
                                    if active_content_for_effect.borrow().is_none() {
                                        let content_owner = parent_owner.child();
                                        let content_transaction = MountTransaction::new();
                                        let content_context = portal_context.with_parts(
                                            MountTarget::Append(slot.clone()),
                                            portal_context.ancestry().clone(),
                                            content_owner.clone(),
                                            content_transaction.clone(),
                                        );
                                        let result = catch_unwind(AssertUnwindSafe(|| {
                                            children.mount(&content_context)
                                        }));
                                        match result {
                                            Ok(Ok(_instance)) => {
                                                match content_transaction.commit() {
                                                    Ok(()) => {
                                                        *active_content_for_effect.borrow_mut() =
                                                            Some(content_owner);
                                                        Ok(())
                                                    }
                                                    Err(error) => {
                                                        let _ = close_owner(&content_owner);
                                                        Err(error)
                                                    }
                                                }
                                            }
                                            Ok(Err(error)) => {
                                                let _ = content_transaction.rollback();
                                                close_owner(&content_owner)?;
                                                Err(error)
                                            }
                                            Err(panic) => {
                                                let _ = content_transaction.rollback();
                                                let _ = close_owner(&content_owner);
                                                resume_unwind(panic);
                                            }
                                        }?
                                    }
                                } else if let Some(content_owner) =
                                    active_content_for_effect.borrow_mut().take()
                                {
                                    close_owner(&content_owner)?;
                                }
                                Ok(())
                            }),
                            error_handler,
                        )
                    })?;
                }
            }

            Ok(())
        }));

        match setup_result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                let _ = close_owner(&host_owner);
                let _ = remove_node(&host);
                return Err(error);
            }
            Err(panic) => {
                let _ = close_owner(&host_owner);
                let _ = remove_node(&host);
                resume_unwind(panic);
            }
        }

        let cleanup_active = Rc::new(Cell::new(true));
        let cleanup_active_for_owner = cleanup_active.clone();
        let cleanup_owner = host_owner.clone();
        let cleanup_host = host.clone();
        owner
            .on_cleanup(
                Box::new(move || {
                    if cleanup_active_for_owner.replace(false) {
                        let owner_result = close_owner(&cleanup_owner);
                        let remove_result = remove_node(&cleanup_host);
                        owner_result?;
                        remove_result?;
                    }
                    Ok(())
                }),
                error_handler,
            )
            .inspect_err(|_| {
                let _ = close_owner(&host_owner);
                let _ = remove_node(&host);
            })?;

        Ok(MountInstance::from_nodes(vec![host]))
    }
}

impl<'scope> View<'scope> for PortalView<'scope> {
    fn mount(&self, context: &MountContext<'scope>) -> SilexResult<MountInstance<'scope>> {
        self.clone().mount_inner(context)
    }
}

/// Create a Portal host that is always mounted for the lifetime of its owner.
#[component]
pub fn PortalHost<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    #[prop(render)]
    #[chain]
    children: AnyView<'scope>,
    #[chain(default)] host_attrs: PortalHostAttrs<'scope>,
    #[chain(default)] mount_to: Option<Node>,
) -> impl View<'scope> {
    PortalView {
        children,
        open: None,
        content_mode: PortalContentMode::KeepAlive,
        mount_to,
        host_attrs,
    }
}

/// Create a stable Portal host whose visibility follows `open`.
#[component]
pub fn Portal<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    #[prop(into)] open: Rx<'scope, bool>,
    #[prop(render)]
    #[chain]
    children: AnyView<'scope>,
    #[chain(default)] host_attrs: PortalHostAttrs<'scope>,
    #[chain(default)] content_mode: PortalContentMode,
    #[chain(default)] mount_to: Option<Node>,
) -> impl View<'scope> {
    PortalView {
        children,
        open: Some(open),
        content_mode,
        mount_to,
        host_attrs,
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn visibility_root_writes_closed_and_open_state_while_detached() {
        let root = PortalVisibilityRoot::create(false).expect("root should be created");
        let element = root.element();

        assert!(element.parent_node().is_none());
        assert!(element.has_attribute(PORTAL_VISIBILITY_ROOT_ATTRIBUTE));
        assert_eq!(
            element.get_attribute("data-state").as_deref(),
            Some("closed")
        );
        assert_eq!(
            element.get_attribute("aria-hidden").as_deref(),
            Some("true")
        );
        assert_eq!(
            element
                .style()
                .get_property_value("display")
                .expect("display should be readable"),
            "none"
        );
        assert_eq!(
            element.style().get_property_priority("display"),
            "important"
        );
        assert_eq!(
            element
                .style()
                .get_property_value("pointer-events")
                .expect("pointer-events should be readable"),
            "none"
        );

        root.set_open(true).expect("root should open");
        assert_eq!(element.get_attribute("data-state").as_deref(), Some("open"));
        assert_eq!(
            element.get_attribute("aria-hidden").as_deref(),
            Some("false")
        );
        assert_eq!(
            element
                .style()
                .get_property_value("display")
                .expect("display should be readable"),
            "contents"
        );
        assert_eq!(
            element
                .style()
                .get_property_value("pointer-events")
                .expect("pointer-events should be readable"),
            "auto"
        );
    }
}
