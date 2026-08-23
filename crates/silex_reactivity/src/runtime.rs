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
pub(crate) use model::{CleanupTarget, ScopePhase, ScopeState, ScopeStateInner};
pub(crate) use ops::{
    acquire_error_handler_lease, commit_signal, invoke_callback, invoke_error_handler,
    node_ref_clear, node_ref_get, node_ref_set, notify, read_fallible_signal_lease,
    read_fallible_signal_lease_untracked, read_signal_lease, read_signal_lease_untracked,
    read_stored_lease, stop_effect, track_fallible_signal, track_signal, track_stored,
    update_signal, update_stored, with_batch, with_fallible_signal, with_fallible_signal_untracked,
    with_runtime, with_signal, with_signal_untracked, with_stored, with_untracked,
    write_signal_lease, write_stored_lease,
};
pub(crate) use scheduler::{
    CloseReportQueue, GlobalScheduler, ObserverFrame, OwnerId, OwnerMode, TargetNode,
};

use crate::error::{ReactiveError, ReactiveResult};
use crate::owner::{self, OwnerHandle};
use crate::root::{CleanupFailure, CloseError, TransientScopeError, TransientScopeResult};

use std::{cell::Cell, marker::PhantomData, rc::Rc};

/// User-owned single-threaded runtime.
pub struct Runtime {
    root_active: Rc<Cell<bool>>,
    close_reports: Rc<CloseReportQueue>,
    marker: PhantomData<Rc<()>>,
}

pub(crate) fn finish_guard<'scope>(
    state: &ScopeState<'scope>,
    should_flush: bool,
) -> ReactiveResult<()> {
    if should_flush {
        eval::flush_if_idle(state)
    } else {
        Ok(())
    }
}

pub(crate) fn report_guard_failure<'scope>(state: &ScopeState<'scope>, error: ReactiveError) {
    let reporter = state.try_borrow().ok().and_then(|state_ref| {
        state_ref
            .scheduler
            .try_borrow()
            .ok()
            .map(|scheduler| scheduler.close_reports.clone())
    });
    if let Some(reporter) = reporter
        && let Some(error) = CloseError::from_failures(vec![CleanupFailure::Runtime(error)])
    {
        reporter.push(error);
    }
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
