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
pub(crate) use input::{
    create_computed, create_computed_always, create_effect, create_effect_detached,
    create_previous, create_watch,
};
#[cfg(feature = "test-support")]
pub use model::RuntimeSnapshot;
pub(crate) use model::{ScopePhase, ScopeState, ScopeStateInner};
pub(crate) use ops::{
    acquire_error_handler_lease, invoke_callback, invoke_error_handler, node_ref_clear,
    node_ref_get, node_ref_set, notify, stop_effect, update_signal, update_stored, with_batch,
    with_fallible_signal, with_signal, with_stored, with_untracked,
};
pub(crate) use scheduler::{CloseReportQueue, GlobalScheduler, ObserverFrame, OwnerId, OwnerMode};

use crate::error::{ReactiveError, ReactiveResult};
use crate::owner::{self, OwnerHandle};
use crate::root::{CloseError, TransientScopeError, TransientScopeResult};

use std::{cell::Cell, marker::PhantomData, rc::Rc};

/// User-owned single-threaded runtime.
pub struct Runtime {
    root_active: Rc<Cell<bool>>,
    close_reports: Rc<CloseReportQueue>,
    marker: PhantomData<Rc<()>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            root_active: Rc::new(Cell::new(false)),
            close_reports: CloseReportQueue::new(),
            marker: PhantomData,
        }
    }

    /// Create the unified persistent root owner.
    pub fn owner(&mut self) -> ReactiveResult<OwnerHandle> {
        if self.root_active.get() {
            return Err(ReactiveError::RuntimeAlreadyRunning);
        }
        self.root_active.set(true);
        owner::new_root(self.root_active.clone(), self.close_reports.clone())
    }

    /// Execute a transient owner whose handles cannot escape the callback.
    pub fn with_transient<R>(
        &mut self,
        f: impl for<'scope> FnOnce(owner::OwnerAccess<'scope>) -> R,
    ) -> TransientScopeResult<R> {
        if self.root_active.get() {
            return Err(TransientScopeError::Runtime(
                ReactiveError::RuntimeAlreadyRunning,
            ));
        }
        owner::new_transient(f, self.close_reports.clone())
    }

    /// Take close diagnostics that originated in Drop or panic recovery paths.
    ///
    /// # Errors
    ///
    /// Returns [`ReactiveError::BorrowConflict`] when another close-report
    /// operation currently holds the queue's dynamic borrow.
    pub fn take_unhandled_close_errors(&self) -> ReactiveResult<Vec<CloseError>> {
        self.close_reports.take()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
