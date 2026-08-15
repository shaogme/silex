//! Explicit runtime storage and the scoped execution driver.
//!
//! A `Runtime` is only an execution boundary. Each `run` creates a fresh
//! reference-counted state whose computation payloads are parameterized by the
//! lexical scope lifetime. Handles keep a safe reference to their scope
//! storage, while the scheduler registry uses `Weak` references for
//! cross-scope lookup.

mod dispose;
mod eval;
mod graph;
mod input;
mod model;
mod ops;
mod scheduler;
pub(crate) mod storage;

pub(crate) use dispose::{dispose_all, dispose_nodes};
pub(crate) use eval::run_global_queue;
pub(crate) use input::{create_derived, create_effect, create_memo, create_previous, create_watch};
#[cfg(feature = "test-support")]
pub use model::RuntimeSnapshot;
pub(crate) use model::{ScopeState, ScopeStateInner};
pub(crate) use ops::{
    invoke_callback, invoke_error_handler, node_ref_clear, node_ref_get, node_ref_set, notify,
    stop_effect, update_signal, update_stored, with_batch, with_fallible_signal, with_signal,
    with_stored, with_untracked,
};
pub(crate) use scheduler::{GlobalScheduler, ObserverFrame, ScopeId};

use crate::scope::ScopeStorage;
use crate::{Scope, error::ReactiveError, root::RootHandle};

use std::{
    cell::Cell,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

/// User-owned single-threaded runtime.
pub struct Runtime {
    root_active: Rc<Cell<bool>>,
    marker: PhantomData<Rc<()>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            root_active: Rc::new(Cell::new(false)),
            marker: PhantomData,
        }
    }

    /// Create one long-lived root owned by the returned handle.
    pub fn run(&mut self) -> Result<RootHandle, ReactiveError> {
        if self.root_active.get() {
            return Err(ReactiveError::RuntimeAlreadyRunning);
        }

        self.root_active.set(true);
        Ok(RootHandle::new(self.root_active.clone()))
    }

    pub fn child<R>(
        &mut self,
        f: impl for<'scope> FnOnce(Scope<'scope>) -> R,
    ) -> Result<R, ReactiveError> {
        if self.root_active.get() {
            return Err(ReactiveError::RuntimeAlreadyRunning);
        }
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler.clone());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let observer_frame = ObserverFrame::push_untracked(scheduler);
        let result = catch_unwind(AssertUnwindSafe(|| f(scope)));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| storage.dispose_untracked()));
        drop(observer_frame);
        match (result, dispose_result) {
            (Ok(value), Ok(Ok(()))) => Ok(value),
            (Ok(_), Ok(Err(_))) => Err(ReactiveError::BorrowConflict),
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
