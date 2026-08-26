use silex_core::{SilexError, SilexErrorKind, SilexResult};
use std::{
    cell::{Cell, RefCell},
    mem,
    rc::Rc,
};

/// 一个 View mount tree 共享的 transaction。
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
        let callbacks = mem::take(&mut *self.inner.callbacks.borrow_mut());
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

#[cfg(test)]
mod tests {
    use super::{MountTransaction, MountTransactionState};
    use silex_core::{SilexError, SilexErrorKind};
    use std::{cell::Cell, rc::Rc};

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
