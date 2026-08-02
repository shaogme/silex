//! Lexical scope capabilities and lifetime boundaries.

use crate::{
    internal::value::OnceThunk,
    runtime::{self, GlobalScheduler, ScopeId, ScopeState, run_global_queue},
};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

/// Stable per-scope metadata referenced by copyable handles.
pub(crate) struct ScopeFrame<'scope> {
    pub(crate) scope_id: ScopeId,
    pub(crate) state: Rc<RefCell<ScopeState<'scope>>>,
}

impl<'scope> ScopeFrame<'scope> {
    pub(crate) fn new(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        let state = Rc::new(RefCell::new(ScopeState::new(ScopeId(0), scheduler.clone())));
        let scope_id = scheduler.borrow_mut().alloc_scope(&state);
        state.borrow_mut().scope_id = scope_id;
        Self { scope_id, state }
    }

    pub(crate) fn dispose(&self) {
        let scheduler = self.state.borrow().scheduler.clone();
        scheduler.borrow_mut().deactivate_scope(self.scope_id);
        runtime::dispose_all(&self.state);
        let should_flush = scheduler.borrow().should_flush();
        if should_flush {
            run_global_queue(&scheduler);
        }
    }
}

/// A copyable capability to create and operate nodes in one lexical scope.
///
/// The scope itself does not own runtime state. The enclosing `ScopeFrame` on
/// the stack manages lexical lifetime, which makes copying this capability
/// harmless and prevents a copied value from disposing the original scope early.
///
/// Child node capabilities cannot be returned from the higher-ranked child
/// callback:
///
/// ```compile_fail
/// use silex_reactivity::Runtime;
///
/// let mut runtime = Runtime::new();
/// let _escaped = runtime.run(|scope| scope.scope(|child| child.signal(0i32).0));
/// ```
#[derive(Clone, Copy)]
pub struct Scope<'scope, 'run> {
    pub(crate) frame: &'scope ScopeFrame<'run>,
    pub(crate) _marker: PhantomData<fn() -> &'scope ()>,
}

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Execute a child scope. All child nodes and computations are destroyed
    /// before this method returns, including during panic unwinding.
    pub fn scope<R>(&self, f: impl for<'s1, 's2, 's3> FnOnce(&'s1 Scope<'s2, 's3>) -> R) -> R {
        let scheduler = self.frame.state.borrow().scheduler.clone();
        let frame = ScopeFrame::new(scheduler);
        let child = Scope {
            frame: &frame,
            _marker: PhantomData,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&child)));
        frame.dispose();
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }

    /// Run a closure without recording signal dependencies. Ownership is
    /// unchanged because only the shared observer slot is modified.
    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> R {
        runtime::with_untracked(&self.frame.state, f)
    }

    /// Defer effect queue flushing until the outermost batch returns.
    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        runtime::with_batch(&self.frame.state, f)
    }

    /// Register cleanup on the current effect, or on this scope when no
    /// computation is active.
    pub fn on_cleanup<F>(&self, f: F)
    where
        F: FnOnce() + 'scope,
    {
        let thunk = OnceThunk::new(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与清理回调，
        // 因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let mut state = self
            .frame
            .state
            .try_borrow_mut()
            .expect("ScopeState borrow failed during on_cleanup registration");
        state.register_cleanup(thunk);
    }
}
