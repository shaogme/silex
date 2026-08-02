//! Explicit runtime storage and the scoped execution driver.
//!
//! A `Runtime` is only an execution boundary. Each `run` creates a fresh
//! reference-counted state whose computation payloads are parameterized by the
//! lifetime of that run. Handles keep only a `Weak` reference to that state, so
//! dropping a scope invalidates every node without leaving an owning cycle.

mod dispose;
mod eval;
mod graph;
mod model;
mod ops;
mod scheduler;

pub(crate) use dispose::dispose_all;
pub(crate) use eval::{run_global_queue, run_initial};
pub(crate) use model::ScopeState;
pub(crate) use ops::{
    invoke_callback, node_ref_get, node_ref_set, notify, track, track_many, update_signal,
    update_stored, with_batch, with_signal, with_stored, with_untracked,
};
pub(crate) use scheduler::{GlobalScheduler, ScopeId};

use crate::scope::{Scope, ScopeFrame};
use std::{
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

/// User-owned single-threaded runtime.
pub struct Runtime {
    running: bool,
    marker: PhantomData<Rc<()>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            running: false,
            marker: PhantomData,
        }
    }

    /// Run one explicit root scope. The higher-ranked callback prevents root
    /// nodes from being returned after this run ends.
    pub fn run<R>(&mut self, f: impl for<'s1, 's2> FnOnce(&'s1 Scope<'s1, 's2>) -> R) -> R {
        assert!(!self.running, "响应式 Runtime 不支持嵌套 run");
        self.running = true;
        let scheduler = GlobalScheduler::new();
        let frame = ScopeFrame::new(scheduler);
        let scope = Scope {
            frame: &frame,
            _marker: PhantomData,
        };
        let result = catch_unwind(AssertUnwindSafe(|| f(&scope)));
        frame.dispose();
        self.running = false;
        match result {
            Ok(value) => value,
            Err(panic) => resume_unwind(panic),
        }
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
