use silex_core::{ErrorReporter, SilexError, SilexErrorKind};
use silex_dom::prelude::*;
use silex_dom::view::{MountErrorHandler, MountOwner};
use silex_macros::component;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};
use web_sys::Node;

#[derive(Clone)]
struct PortalView<'scope> {
    children: AnyView<'scope>,
    mount_to: Option<Node>,
}

impl<'scope> PortalView<'scope> {
    fn mount_inner(
        self,
        owner: &dyn MountOwner<'scope>,
        _parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> silex_core::SilexResult<()> {
        let document = silex_dom::document();
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
        let container = document.create_element("div").map_err(SilexError::fatal)?;
        container
            .set_attribute("style", "display: contents")
            .map_err(SilexError::fatal)?;
        let container: Node = container.into();
        target.append_child(&container).map_err(SilexError::fatal)?;

        let active = Rc::new(Cell::new(true));
        let cleanup_active = active.clone();
        let cleanup_target = target.clone();
        let cleanup_container = container.clone();
        let rollback_target = target.clone();
        let rollback_container = container.clone();
        if let Err(error) = owner.on_cleanup(
            Box::new(move || -> silex_core::SilexResult<()> {
                if cleanup_active.replace(false) {
                    cleanup_target
                        .remove_child(&cleanup_container)
                        .map_err(SilexError::fatal)?;
                }
                Ok(())
            }),
            error_handler,
        ) {
            active.set(false);
            let _ = rollback_target.remove_child(&rollback_container);
            return Err(error);
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            self.children
                .mount_owned(owner, &container, attrs, error_handler)
        }));
        match result {
            Err(panic) => {
                if active.replace(false) {
                    let _ = target.remove_child(&container);
                }
                resume_unwind(panic);
            }
            Ok(Err(error)) => {
                if active.replace(false) {
                    let _ = target.remove_child(&container);
                }
                return Err(error);
            }
            Ok(Ok(())) => {}
        }
        Ok(())
    }
}

impl<'scope> View<'scope> for PortalView<'scope> {
    fn mount(
        &self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> silex_core::SilexResult<()> {
        self.clone()
            .mount_inner(owner, parent, attrs, error_handler)
    }

    fn mount_owned(
        self,
        owner: &dyn MountOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
        error_handler: MountErrorHandler<'scope>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount_inner(owner, parent, attrs, error_handler)
    }
}

impl<'scope> ApplyAttributes<'scope> for PortalView<'scope> {}

/// Portal 组件：将子视图渲染到当前 DOM 树之外的节点（默认是 document.body）。
/// 但保持响应式上下文（Context）的连通性。
#[component]
pub fn Portal<'scope>(
    error_handler: ErrorReporter<'scope>,
    #[prop(into)] children: AnyView<'scope>,
    #[chain(default)] mount_to: Option<Node>,
) -> impl View<'scope> {
    PortalView { children, mount_to }
}
