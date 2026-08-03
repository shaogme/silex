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

pub(crate) use dispose::dispose_all;
pub(crate) use eval::{run_global_queue, run_initial};
pub use input::{RuntimeInput, RuntimeInputs};
pub(crate) use input::{create_derived, create_effect, create_memo, validate_inputs};
pub(crate) use model::ScopeState;
pub(crate) use ops::{
    invoke_callback, node_ref_clear, node_ref_get, node_ref_set, notify, track, track_many,
    update_signal, update_stored, with_batch, with_signal, with_stored, with_untracked,
};
pub(crate) use scheduler::{GlobalScheduler, ScopeId};

use crate::scope::ScopeStorage;
use crate::{Scope, root::RootHandle};

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

    /// Run one long-lived root owned by the returned handle.
    pub fn run<F>(&mut self, f: F) -> RootHandle
    where
        F: FnOnce(&crate::RootScope),
    {
        assert!(
            !self.root_active.replace(true),
            "一个 Runtime 只能同时拥有一个 root"
        );

        let mut root = RootHandle::new(self.root_active.clone());
        let scope = root.scope();
        let result = catch_unwind(AssertUnwindSafe(|| f(&scope)));
        if let Err(panic) = result {
            if let Err(cleanup) = root.dispose() {
                cleanup.report_during_unwind();
            }
            resume_unwind(panic);
        }
        root
    }

    pub fn child<R>(&mut self, f: impl for<'scope> FnOnce(&'scope Scope<'scope>) -> R) -> R {
        assert!(
            !self.root_active.get(),
            "长期 root 存活期间不能运行词法测试 scope"
        );
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler);
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let result = catch_unwind(AssertUnwindSafe(|| f(&scope)));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| storage.dispose()));
        match (result, dispose_result) {
            (Ok(value), Ok(())) => value,
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
