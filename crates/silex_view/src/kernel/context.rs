use super::{MountAncestry, MountInstance, MountTarget, MountTransaction, View};
use crate::lifecycle::{MountErrorHandler, MountOwnerToken};
use silex_core::{ReactiveError, SilexError, SilexResult};
use silex_dom::{
    diagnostics::DomResult,
    lifecycle::node_ref::NodeRef,
    model::{DomElement, DomNode},
    runtime::DomContext,
};

/// Owner-bound, backend-neutral capability entry for mount-time DOM actions.
#[derive(Clone)]
pub struct MountDomAction<'scope> {
    dom: DomContext,
    owner: MountOwnerToken<'scope>,
}

impl<'scope> MountDomAction<'scope> {
    pub(crate) fn new(dom: DomContext, owner: MountOwnerToken<'scope>) -> Self {
        Self { dom, owner }
    }

    pub fn with_context<R, F>(&self, action: F) -> SilexResult<R>
    where
        F: FnOnce(&DomContext) -> DomResult<R>,
    {
        if !self.owner.is_active()? {
            return Err(SilexError::fatal(ReactiveError::NoSuchNode));
        }
        action(&self.dom).map_err(Into::into)
    }

    pub fn focus(&self, node_ref: &NodeRef<'scope>) -> SilexResult<()> {
        self.with_context(|dom| node_ref.focus(dom))
    }
}

/// View kernel context。这里的 `MountContext` 不是应用 builder context。
#[derive(Clone)]
pub struct MountContext<'scope> {
    dom: DomContext,
    target: MountTarget,
    ancestry: MountAncestry,
    owner: MountOwnerToken<'scope>,
    transaction: MountTransaction<'scope>,
    error_handler: MountErrorHandler<'scope>,
}

impl<'scope> MountContext<'scope> {
    pub fn for_parent(
        dom: DomContext,
        parent: DomNode,
        owner: MountOwnerToken<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> Self {
        Self::new(
            MountTarget::append(dom, parent),
            MountAncestry::root(),
            owner,
            MountTransaction::new(),
            error_handler,
        )
    }

    pub fn new(
        target: MountTarget,
        ancestry: MountAncestry,
        owner: MountOwnerToken<'scope>,
        transaction: MountTransaction<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> Self {
        Self {
            dom: target.context().clone(),
            target,
            ancestry,
            owner,
            transaction,
            error_handler,
        }
    }

    pub fn dom(&self) -> &DomContext {
        &self.dom
    }

    /// Create an owner-bound DOM action for event callbacks and mount-time work.
    pub fn dom_action(&self) -> MountDomAction<'scope> {
        MountDomAction::new(self.dom.clone(), self.owner.clone())
    }

    pub fn target(&self) -> &MountTarget {
        &self.target
    }

    pub fn ancestry(&self) -> &MountAncestry {
        &self.ancestry
    }

    pub fn owner(&self) -> MountOwnerToken<'scope> {
        self.owner.clone()
    }

    pub fn transaction(&self) -> &MountTransaction<'scope> {
        &self.transaction
    }

    pub fn error_handler(&self) -> MountErrorHandler<'scope> {
        self.error_handler
    }

    pub fn mount<V>(&self, view: &V) -> SilexResult<MountInstance<'scope>>
    where
        V: View<'scope> + ?Sized,
    {
        view.mount(self)
    }

    pub fn mount_unit<V>(&self, view: &V) -> SilexResult<()>
    where
        V: View<'scope> + ?Sized,
    {
        self.mount(view).map(|_| ())
    }

    pub fn on_commit<F>(&self, callback: F) -> SilexResult<()>
    where
        F: FnOnce() -> SilexResult<()> + 'scope,
    {
        let error_handler = self.error_handler;
        self.transaction.on_commit(move || {
            let result = callback();
            if let Err(error) = &result {
                let _ = error_handler.handle(error.clone());
            }
            result
        })
    }

    pub fn with_target(&self, target: MountTarget) -> Self {
        Self::new(
            target,
            self.ancestry.clone(),
            self.owner.clone(),
            self.transaction.clone(),
            self.error_handler,
        )
    }

    pub fn with_owner(&self, owner: MountOwnerToken<'scope>) -> Self {
        Self::new(
            self.target.clone(),
            self.ancestry.clone(),
            owner,
            self.transaction.clone(),
            self.error_handler,
        )
    }

    pub fn with_error_handler(&self, error_handler: MountErrorHandler<'scope>) -> Self {
        Self::new(
            self.target.clone(),
            self.ancestry.clone(),
            self.owner.clone(),
            self.transaction.clone(),
            error_handler,
        )
    }

    pub fn with_element(&self, element: &DomElement) -> Self {
        Self::new(
            MountTarget::append(self.dom.clone(), element.node().clone()),
            self.ancestry.push(element),
            self.owner.clone(),
            self.transaction.clone(),
            self.error_handler,
        )
    }

    pub fn with_parts(
        &self,
        target: MountTarget,
        ancestry: MountAncestry,
        owner: MountOwnerToken<'scope>,
        transaction: MountTransaction<'scope>,
    ) -> Self {
        Self::new(target, ancestry, owner, transaction, self.error_handler)
    }
}
