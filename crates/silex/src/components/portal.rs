use silex_core::{SilexError, SilexErrorKind, SilexResult, reactivity::Signal};
use silex_dom::view::{MountErrorHandler, MountOwner, MountOwnerToken};
use silex_dom::{document, prelude::*};
use silex_macros::component;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};

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
    open: Option<Signal<'scope, bool>>,
    content_mode: PortalContentMode,
    mount_to: Option<Node>,
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

fn update_host_state(host: &HtmlElement, open: bool) -> SilexResult<()> {
    let element: &Element = host.as_ref();
    element
        .set_attribute("data-state", if open { "open" } else { "closed" })
        .map_err(SilexError::fatal)?;
    element
        .set_attribute("aria-hidden", if open { "false" } else { "true" })
        .map_err(SilexError::fatal)?;
    if open {
        element
            .remove_attribute("hidden")
            .map_err(SilexError::fatal)?;
    } else {
        element
            .set_attribute("hidden", "")
            .map_err(SilexError::fatal)?;
    }
    host.style()
        .set_property("pointer-events", if open { "auto" } else { "none" })
        .map_err(SilexError::fatal)
}

impl<'scope> PortalView<'scope> {
    fn mount_inner(
        self,
        owner: &dyn MountOwner<'scope>,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
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
        let host_element = document.create_element("div").map_err(SilexError::fatal)?;
        let host: Node = host_element.clone().into();
        let host_owner = owner.child();
        let host_token = host_owner.clone();

        let setup_result = catch_unwind(AssertUnwindSafe(|| -> SilexResult<()> {
            for attr in attrs {
                attr.apply(&host_element, &host_token, error_handler)?;
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
            target.append_child(&host).map_err(SilexError::fatal)?;

            if let Some(open) = self.open {
                let host_html = host_element
                    .clone()
                    .dyn_into::<HtmlElement>()
                    .map_err(|_| {
                        SilexError::fatal(SilexErrorKind::Dom(
                            "Portal host must be an HTML element".to_string(),
                        ))
                    })?;
                host_owner.effect(
                    Box::new(move || {
                        let visible = open.with(|value| *value)?;
                        update_host_state(&host_html, visible)
                    }),
                    error_handler,
                )?;
            }

            match self.content_mode {
                PortalContentMode::KeepAlive => {
                    let content_owner = host_owner.child();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        self.children
                            .mount(&content_owner, &host, Vec::new(), error_handler)
                    }));
                    match result {
                        Ok(Ok(_instance)) => {}
                        Ok(Err(error)) => {
                            close_owner(&content_owner)?;
                            return Err(error);
                        }
                        Err(panic) => {
                            let _ = close_owner(&content_owner);
                            resume_unwind(panic);
                        }
                    }
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
                    host.append_child(&slot).map_err(SilexError::fatal)?;

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
                    host_owner.effect(
                        Box::new(move || -> SilexResult<()> {
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
                                    let result = catch_unwind(AssertUnwindSafe(|| {
                                        children.mount(
                                            &content_owner,
                                            &slot,
                                            Vec::new(),
                                            error_handler,
                                        )
                                    }));
                                    match result {
                                        Ok(Ok(_instance)) => {
                                            *active_content_for_effect.borrow_mut() =
                                                Some(content_owner);
                                        }
                                        Ok(Err(error)) => {
                                            close_owner(&content_owner)?;
                                            return Err(error);
                                        }
                                        Err(panic) => {
                                            let _ = close_owner(&content_owner);
                                            resume_unwind(panic);
                                        }
                                    }
                                }
                            } else if let Some(content_owner) =
                                active_content_for_effect.borrow_mut().take()
                            {
                                close_owner(&content_owner)?;
                            }
                            Ok(())
                        }),
                        error_handler,
                    )?;
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
                        if let Err(error) = owner_result {
                            return Err(error);
                        }
                        remove_result?;
                    }
                    Ok(())
                }),
                error_handler,
            )
            .map_err(|error| {
                let _ = close_owner(&host_owner);
                let _ = remove_node(&host);
                error
            })?;

        Ok(MountInstance::from_nodes(vec![host]))
    }
}

impl<'scope> View<'scope> for PortalView<'scope> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        attrs: Vec<AttrOp<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> SilexResult<MountInstance<'scope>> {
        self.clone().mount_inner(owner, attrs, error_handler)
    }
}

impl<'scope> ApplyAttributes<'scope> for PortalView<'scope> {}

/// Create a Portal host that is always mounted for the lifetime of its owner.
#[component]
pub fn PortalHost<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    #[prop(render)]
    #[chain]
    children: AnyView<'scope>,
    #[chain(default)] mount_to: Option<Node>,
) -> impl View<'scope> {
    PortalView {
        children,
        open: None,
        content_mode: PortalContentMode::KeepAlive,
        mount_to,
    }
}

/// Create a stable Portal host whose visibility follows `open`.
#[component]
pub fn Portal<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    #[prop(into)] open: Signal<'scope, bool>,
    #[prop(render)]
    #[chain]
    children: AnyView<'scope>,
    #[chain(default)] content_mode: PortalContentMode,
    #[chain(default)] mount_to: Option<Node>,
) -> impl View<'scope> {
    PortalView {
        children,
        open: Some(open),
        content_mode,
        mount_to,
    }
}
