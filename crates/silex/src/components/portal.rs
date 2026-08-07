use silex_dom::prelude::*;
use silex_dom::view::ViewOwner;
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
        owner: &dyn ViewOwner<'scope>,
        _parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        let document = silex_dom::document();
        let target = self.mount_to.unwrap_or_else(|| {
            document
                .body()
                .expect("Portal requires document.body when no target is supplied")
                .into()
        });
        let container = document
            .create_element("div")
            .map_err(silex_core::SilexError::from)?;
        container
            .set_attribute("style", "display: contents")
            .map_err(silex_core::SilexError::from)?;
        let container: Node = container.into();
        target
            .append_child(&container)
            .map_err(silex_core::SilexError::from)?;

        let active = Rc::new(Cell::new(true));
        let cleanup_active = active.clone();
        let cleanup_target = target.clone();
        let cleanup_container = container.clone();
        owner.on_cleanup(Box::new(move || {
            if cleanup_active.replace(false) {
                let _ = cleanup_target.remove_child(&cleanup_container);
            }
        }))?;

        let result = catch_unwind(AssertUnwindSafe(|| {
            self.children.mount_owned(owner, &container, attrs)
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
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()> {
        self.clone().mount_inner(owner, parent, attrs)
    }

    fn mount_owned(
        self,
        owner: &dyn ViewOwner<'scope>,
        parent: &Node,
        attrs: Vec<PendingAttribute<'scope>>,
    ) -> silex_core::SilexResult<()>
    where
        Self: Sized,
    {
        self.mount_inner(owner, parent, attrs)
    }
}

impl<'scope> ApplyAttributes<'scope> for PortalView<'scope> {}

/// Portal 组件：将子视图渲染到当前 DOM 树之外的节点（默认是 document.body）。
/// 但保持响应式上下文（Context）的连通性。
#[component]
pub fn Portal<'scope>(
    #[prop(into)] children: AnyView<'scope>,
    #[chain(default)] mount_to: Option<Node>,
) -> impl View<'scope> {
    PortalView { children, mount_to }
}
