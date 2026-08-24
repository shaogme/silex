use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};

use crate::error::DomResult;

/// Host capabilities consumed by low-level DOM adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostCapability {
    EventListener,
    Timer,
    AnimationFrame,
    IdleCallback,
    Microtask,
}

/// Idempotent resource lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostResourceState {
    Active,
    Finished,
    Cancelled,
    Inert,
}

type CancelAction = Box<dyn FnOnce() -> DomResult<()> + 'static>;

struct HostResourceInner {
    active: Cell<bool>,
    state: Cell<HostResourceState>,
    cancel: RefCell<Option<CancelAction>>,
}

/// Owned host resource with gate-first, exactly-once cancellation.
pub struct HostResource<'scope> {
    inner: Rc<HostResourceInner>,
    marker: PhantomData<fn(&'scope ())>,
}

impl HostResource<'static> {
    #[cfg(any(feature = "browser", test))]
    pub(crate) fn with_cancel<F>(cancel: F) -> Self
    where
        F: FnOnce() -> DomResult<()> + 'static,
    {
        Self {
            inner: Rc::new(HostResourceInner {
                active: Cell::new(true),
                state: Cell::new(HostResourceState::Active),
                cancel: RefCell::new(Some(Box::new(cancel))),
            }),
            marker: PhantomData,
        }
    }

    pub(crate) fn inert() -> Self {
        Self {
            inner: Rc::new(HostResourceInner {
                active: Cell::new(false),
                state: Cell::new(HostResourceState::Inert),
                cancel: RefCell::new(None),
            }),
            marker: PhantomData,
        }
    }
}

impl<'scope> HostResource<'scope> {
    pub fn state(&self) -> HostResourceState {
        self.inner.state.get()
    }

    pub fn is_active(&self) -> bool {
        self.inner.active.get() && self.state() == HostResourceState::Active
    }

    pub fn finish(&self) {
        if self.inner.state.replace(HostResourceState::Finished) == HostResourceState::Active {
            self.inner.active.set(false);
            let _ = self.inner.cancel.borrow_mut().take();
        }
    }

    pub fn cancel(&self) -> DomResult<()> {
        if self.inner.state.replace(HostResourceState::Cancelled) != HostResourceState::Active {
            return Ok(());
        }

        self.inner.active.set(false);
        let action = self.inner.cancel.borrow_mut().take();
        match action {
            Some(action) => action(),
            None => Ok(()),
        }
    }
}

impl Drop for HostResource<'_> {
    fn drop(&mut self) {
        let _ = self.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::{HostResource, HostResourceState};
    use crate::error::DomResult;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn cancel_closes_gate_before_running_action_and_is_idempotent() {
        let count = Rc::new(Cell::new(0));
        let count_for_action = count.clone();
        let resource = HostResource::with_cancel(move || {
            count_for_action.set(count_for_action.get() + 1);
            Ok(())
        });
        assert!(resource.is_active());
        resource.cancel().expect("first cancel should succeed");
        resource.cancel().expect("second cancel should be inert");
        assert_eq!(count.get(), 1);
        assert_eq!(resource.state(), HostResourceState::Cancelled);
    }

    #[test]
    fn action_error_does_not_reopen_resource() {
        let resource = HostResource::with_cancel(|| -> DomResult<()> {
            Err(crate::error::DomError::Backend {
                operation: "cancel",
                message: String::from("failed"),
            })
        });
        assert!(resource.cancel().is_err());
        assert!(!resource.is_active());
        assert_eq!(resource.state(), HostResourceState::Cancelled);
        resource.cancel().expect("repeated cancel should be inert");
    }
}
