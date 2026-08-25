use crate::owner::{MountErrorHandler, MountOwnerToken};
use crate::{MountInstance, View};
use silex_core::{ReactiveError, SilexError, SilexErrorKind, SilexResult};
use silex_dom::{
    diagnostics::{DomError, DomResult},
    lifecycle::node_ref::NodeRef,
    model::{DomElement, DomNode},
    runtime::{DomContext, InsertRequest},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// View 写入的物理目标。
#[derive(Clone)]
pub enum MountTarget {
    Append {
        context: DomContext,
        parent: DomNode,
    },
    Before {
        context: DomContext,
        reference: DomNode,
    },
}

impl MountTarget {
    pub fn append(context: DomContext, parent: DomNode) -> Self {
        Self::Append { context, parent }
    }

    pub fn before(context: DomContext, reference: DomNode) -> Self {
        Self::Before { context, reference }
    }

    pub fn context(&self) -> &DomContext {
        match self {
            Self::Append { context, .. } | Self::Before { context, .. } => context,
        }
    }

    pub fn append_node(&self, node: &DomNode) -> SilexResult<()> {
        match self {
            Self::Append { context, parent } => context.append(parent, node).map_err(Into::into),
            Self::Before { context, reference } => {
                let parent = context
                    .parent(reference)
                    .map_err(SilexError::from)?
                    .ok_or_else(|| SilexError::from(DomError::NoParent))?;
                context
                    .insert_before(InsertRequest::before(&parent, node, reference))
                    .map_err(Into::into)
            }
        }
    }

    pub fn parent(&self) -> SilexResult<DomNode> {
        match self {
            Self::Append { parent, .. } => Ok(parent.clone()),
            Self::Before { context, reference } => context
                .parent(reference)
                .map_err(SilexError::from)?
                .ok_or_else(|| SilexError::from(DomError::NoParent)),
        }
    }
}

struct AncestryLink {
    element: DomElement,
    parent: Option<Rc<AncestryLink>>,
}

/// 与物理 parent chain 分离的逻辑 element ancestry。
#[derive(Clone, Default)]
pub struct MountAncestry {
    current: Option<Rc<AncestryLink>>,
}

impl MountAncestry {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn push(&self, element: &DomElement) -> Self {
        Self {
            current: Some(Rc::new(AncestryLink {
                element: element.clone(),
                parent: self.current.clone(),
            })),
        }
    }

    pub fn current_element(&self) -> Option<DomElement> {
        self.current.as_ref().map(|link| link.element.clone())
    }

    pub fn find_element<F>(&self, mut predicate: F) -> Option<DomElement>
    where
        F: FnMut(&DomElement) -> bool,
    {
        let mut current = self.current.clone();
        while let Some(link) = current {
            if predicate(&link.element) {
                return Some(link.element.clone());
            }
            current = link.parent.clone();
        }
        None
    }

    pub fn closest_logical_element(&self, _selector: &str) -> SilexResult<Option<DomElement>> {
        Err(SilexError::from(DomError::Unsupported {
            capability: "logical selector matching",
        }))
    }
}

/// 一个 View mount tree 共享的 transaction。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountTransactionState {
    Open,
    Committed,
    RolledBack,
}

/// Owner-bound, backend-neutral capability entry for mount-time DOM actions.
///
/// The action never stores a node reference or a browser value. Callers must
/// provide the operation explicitly through [`Self::with_context`], and the
/// owner activity gate is checked before the operation runs.
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

type CommitCallback<'scope> = Box<dyn FnOnce() -> SilexResult<()> + 'scope>;

struct TransactionInner<'scope> {
    state: Cell<MountTransactionState>,
    callbacks: RefCell<Vec<CommitCallback<'scope>>>,
    parent: Option<MountTransaction<'scope>>,
}

#[derive(Clone)]
pub struct MountTransaction<'scope> {
    inner: Rc<TransactionInner<'scope>>,
}

impl<'scope> MountTransaction<'scope> {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(TransactionInner {
                state: Cell::new(MountTransactionState::Open),
                callbacks: RefCell::new(Vec::new()),
                parent: None,
            }),
        }
    }

    pub fn state(&self) -> MountTransactionState {
        self.inner.state.get()
    }

    pub fn child(&self) -> SilexResult<Self> {
        self.ensure_open()?;
        Ok(Self {
            inner: Rc::new(TransactionInner {
                state: Cell::new(MountTransactionState::Open),
                callbacks: RefCell::new(Vec::new()),
                parent: Some(self.clone()),
            }),
        })
    }

    pub fn on_commit<F>(&self, callback: F) -> SilexResult<()>
    where
        F: FnOnce() -> SilexResult<()> + 'scope,
    {
        self.ensure_open()?;
        self.inner.callbacks.borrow_mut().push(Box::new(callback));
        Ok(())
    }

    pub fn commit(&self) -> SilexResult<()> {
        self.ensure_open()?;
        if let Some(parent) = &self.inner.parent {
            parent.accept_callbacks(&mut self.inner.callbacks.borrow_mut())?;
            self.inner.state.set(MountTransactionState::Committed);
            return Ok(());
        }

        self.inner.state.set(MountTransactionState::Committed);
        let callbacks = std::mem::take(&mut *self.inner.callbacks.borrow_mut());
        let mut first_error = None;
        for callback in callbacks {
            if let Err(error) = callback()
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    pub fn rollback(&self) -> SilexResult<()> {
        self.ensure_open()?;
        self.inner.callbacks.borrow_mut().clear();
        self.inner.state.set(MountTransactionState::RolledBack);
        Ok(())
    }

    fn accept_callbacks(&self, callbacks: &mut Vec<CommitCallback<'scope>>) -> SilexResult<()> {
        self.ensure_open()?;
        self.inner.callbacks.borrow_mut().append(callbacks);
        Ok(())
    }

    fn ensure_open(&self) -> SilexResult<()> {
        if self.state() == MountTransactionState::Open {
            return Ok(());
        }
        Err(SilexError::fatal(SilexErrorKind::Framework(format!(
            "mount transaction is already {:?}",
            self.state()
        ))))
    }
}

impl Default for MountTransaction<'_> {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::{MountTransaction, MountTransactionState};
    use silex_core::{SilexError, SilexErrorKind};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn child_commit_waits_for_root_and_rollback_rejects_reuse() {
        let called = Rc::new(Cell::new(false));
        let root = MountTransaction::new();
        let child = root.child().expect("child should open");
        let called_by_child = called.clone();
        child
            .on_commit(move || {
                called_by_child.set(true);
                Ok(())
            })
            .expect("callback should register");
        child.commit().expect("child should merge");
        assert!(!called.get());
        root.commit().expect("root should run callback");
        assert!(called.get());
        assert_eq!(root.state(), MountTransactionState::Committed);

        let transaction = MountTransaction::new();
        transaction
            .on_commit(|| {
                Err(SilexError::fatal(SilexErrorKind::Framework(
                    "callback failure".into(),
                )))
            })
            .expect("callback should register");
        assert!(transaction.commit().is_err());
        assert!(transaction.on_commit(|| Ok(())).is_err());
    }
}
