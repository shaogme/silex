use super::owner::{MountErrorHandler, MountOwnerToken};
use silex_core::{SilexError, SilexErrorKind, SilexResult};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use web_sys::{Element, Node};

/// The physical insertion position used by a view mount.
///
/// The target intentionally contains no logical-parent information. A
/// detached fragment, a portal target, and a normal DOM parent can therefore
/// share the same insertion contract while keeping their logical ancestry
/// independent.
#[derive(Clone)]
pub enum MountTarget {
    Append(Node),
    Before(Node),
}

impl MountTarget {
    pub fn append(&self, node: &Node) -> SilexResult<()> {
        match self {
            Self::Append(parent) => parent.append_child(node),
            Self::Before(reference) => {
                let parent = reference.parent_node().ok_or_else(|| {
                    SilexError::fatal(SilexErrorKind::Dom(
                        "mount target reference has no parent".to_string(),
                    ))
                })?;
                parent.insert_before(node, Some(reference))
            }
        }
        .map(|_| ())
        .map_err(SilexError::fatal)
    }

    pub fn parent(&self) -> SilexResult<Node> {
        match self {
            Self::Append(parent) => Ok(parent.clone()),
            Self::Before(reference) => reference.parent_node().ok_or_else(|| {
                SilexError::fatal(SilexErrorKind::Dom(
                    "mount target reference has no parent".to_string(),
                ))
            }),
        }
    }
}

struct AncestryLink {
    element: Element,
    parent: Option<Rc<AncestryLink>>,
}

/// A persistent logical element ancestry chain.
///
/// This is deliberately separate from the physical DOM parent chain. The
/// chain remains valid while nodes are inside staging fragments or are
/// rendered into a portal target.
#[derive(Clone, Default)]
pub struct MountAncestry {
    current: Option<Rc<AncestryLink>>,
}

impl MountAncestry {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn push(&self, element: &Element) -> Self {
        Self {
            current: Some(Rc::new(AncestryLink {
                element: element.clone(),
                parent: self.current.clone(),
            })),
        }
    }

    pub fn current_element(&self) -> Option<Element> {
        self.current.as_ref().map(|link| link.element.clone())
    }

    pub fn find_element<F>(&self, mut predicate: F) -> Option<Element>
    where
        F: FnMut(&Element) -> bool,
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

    pub fn closest_logical_element(&self, selector: &str) -> SilexResult<Option<Element>> {
        let mut current = self.current.clone();
        while let Some(link) = current {
            if link.element.matches(selector).map_err(SilexError::fatal)? {
                return Ok(Some(link.element.clone()));
            }
            current = link.parent.clone();
        }
        Ok(None)
    }
}

/// The lifecycle state of one mount transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MountTransactionState {
    Open,
    Committed,
    RolledBack,
}

type CommitCallback<'scope> = Box<dyn FnOnce() -> SilexResult<()> + 'scope>;

struct TransactionInner<'scope> {
    state: Cell<MountTransactionState>,
    callbacks: RefCell<Vec<CommitCallback<'scope>>>,
    parent: Option<MountTransaction<'scope>>,
}

/// Commit/rollback state shared by all contexts in one mount tree.
///
/// Child transactions merge callbacks into their parent on commit. Only a
/// root transaction executes callbacks; therefore nested staging work cannot
/// expose a commit side effect before the outer mount succeeds.
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

impl<'scope> Default for MountTransaction<'scope> {
    fn default() -> Self {
        Self::new()
    }
}

/// Context passed through the view mount kernel.
#[derive(Clone)]
pub struct MountContext<'scope> {
    target: MountTarget,
    ancestry: MountAncestry,
    owner: MountOwnerToken<'scope>,
    transaction: MountTransaction<'scope>,
    error_handler: MountErrorHandler<'scope>,
}

impl<'scope> MountContext<'scope> {
    pub fn for_parent(
        parent: Node,
        owner: MountOwnerToken<'scope>,
        error_handler: MountErrorHandler<'scope>,
    ) -> Self {
        Self::new(
            MountTarget::Append(parent),
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
            target,
            ancestry,
            owner,
            transaction,
            error_handler,
        }
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

    pub fn with_element(&self, element: &Element) -> Self {
        Self::new(
            MountTarget::Append(element.clone().into()),
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
    use super::*;
    use std::cell::Cell;

    #[test]
    fn nested_commit_callbacks_wait_for_root_commit() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let root = MountTransaction::new();
        let child = root.child().expect("child transaction should open");
        let child_events = events.clone();
        child
            .on_commit(move || {
                child_events.borrow_mut().push("child");
                Ok(())
            })
            .expect("callback should register");

        child.commit().expect("child commit should merge callbacks");
        assert!(events.borrow().is_empty());
        assert_eq!(child.state(), MountTransactionState::Committed);
        assert_eq!(root.state(), MountTransactionState::Open);

        root.commit().expect("root commit should run callbacks");
        assert_eq!(&*events.borrow(), &["child"]);
    }

    #[test]
    fn rollback_cancels_callbacks_and_rejects_reuse() {
        let called = Rc::new(Cell::new(false));
        let transaction = MountTransaction::new();
        let called_for_callback = called.clone();
        transaction
            .on_commit(move || {
                called_for_callback.set(true);
                Ok(())
            })
            .expect("callback should register");

        transaction.rollback().expect("rollback should succeed");
        assert_eq!(transaction.state(), MountTransactionState::RolledBack);
        assert!(!called.get());
        assert!(transaction.commit().is_err());
        assert!(transaction.rollback().is_err());
    }

    #[test]
    fn commit_runs_each_callback_once_even_when_one_fails() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let transaction = MountTransaction::new();
        let first_events = events.clone();
        transaction
            .on_commit(move || {
                first_events.borrow_mut().push("first");
                Err(SilexError::fatal(SilexErrorKind::Framework(
                    "commit failure".to_string(),
                )))
            })
            .expect("first callback should register");
        let second_events = events.clone();
        transaction
            .on_commit(move || {
                second_events.borrow_mut().push("second");
                Ok(())
            })
            .expect("second callback should register");

        assert!(transaction.commit().is_err());
        assert_eq!(&*events.borrow(), &["first", "second"]);
        assert!(transaction.on_commit(|| Ok(())).is_err());
    }
}
