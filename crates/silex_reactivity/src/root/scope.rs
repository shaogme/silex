//! Long-lived root owner and cleanup error handling.

use crate::{Scope, runtime::GlobalScheduler, scope::ScopeStorage};
use std::{
    cell::Cell,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

/// A cleanup failure returned by an explicit root disposal.
pub struct CleanupError {
    panic: Box<dyn std::any::Any + Send>,
}

impl fmt::Debug for CleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CleanupError").finish_non_exhaustive()
    }
}

impl CleanupError {
    fn new(panic: Box<dyn std::any::Any + Send>) -> Self {
        Self { panic }
    }

    fn resume(self) -> ! {
        resume_unwind(self.panic)
    }
}

/// Owns one long-lived root storage.
pub struct RootHandle {
    storage: Rc<ScopeStorage>,
    runtime_slot: Rc<Cell<bool>>,
    disposed: bool,
}

impl RootHandle {
    pub(crate) fn new(runtime_slot: Rc<Cell<bool>>) -> Self {
        Self {
            storage: Rc::new(ScopeStorage::new(GlobalScheduler::new())),
            runtime_slot,
            disposed: false,
        }
    }

    /// Borrow the ordinary scope capability used to create root nodes.
    pub fn scope(&self) -> Scope<'_> {
        Scope {
            storage: self.storage.as_ref(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Execute a callback with a root scope borrowed for exactly this owner.
    pub fn with_scope<'scope, R>(&'scope self, f: impl FnOnce(Scope<'scope>) -> R) -> R {
        let scope = Scope {
            storage: self.storage.as_ref(),
            _marker: std::marker::PhantomData,
        };
        f(scope)
    }

    /// Dispose the root exactly once.
    pub fn dispose(mut self) -> Result<(), CleanupError> {
        self.dispose_inner()
    }

    pub fn is_active(&self) -> bool {
        !self.disposed
            && self
                .storage
                .state
                .borrow()
                .scheduler
                .borrow()
                .is_scope_active(self.storage.scope_id)
    }

    fn dispose_inner(&mut self) -> Result<(), CleanupError> {
        if self.disposed {
            self.runtime_slot.set(false);
            return Ok(());
        }

        self.disposed = true;
        self.runtime_slot.set(false);
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.storage.dispose_untracked();
        }));

        match result {
            Ok(()) => Ok(()),
            Err(panic) => Err(CleanupError::new(panic)),
        }
    }
}

impl Drop for RootHandle {
    fn drop(&mut self) {
        if let Err(error) = self.dispose_inner() {
            error.resume();
        }
    }
}
