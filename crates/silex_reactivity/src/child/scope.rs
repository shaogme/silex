//! Lexical scope capabilities and lifetime boundaries.

use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    mem::transmute,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

use super::node::{
    Callback, Derived, Effect, Memo, NodeRef, ReadSignal, Signal, StoredValue, WriteSignal,
};
use crate::{
    handle::Handle,
    internal::value::{AnyValue, CallbackThunk, EffectThunk, MemoThunk, OnceThunk},
    runtime,
    scope::ScopeFrame,
};

/// A copyable capability to create and operate nodes in one lexical scope.
///
/// The scope itself does not own runtime state. The enclosing `ScopeFrame` on
/// the stack manages lexical lifetime, which makes copying this capability
/// harmless and prevents a copied value from disposing the original scope early.
///
/// Child node capabilities cannot be returned from the higher-ranked child
/// callback. The compile-fail case is covered by
/// `tests/ui/fail_child_handle_escape.rs`.
#[derive(Clone, Copy)]
pub struct Scope<'scope, 'run> {
    pub(crate) frame: &'scope ScopeFrame<'run>,
    pub(crate) _marker: PhantomData<fn() -> &'scope ()>,
}

impl<'scope, 'run> PartialEq for Scope<'scope, 'run> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.frame, other.frame)
    }
}

impl<'scope, 'run> Eq for Scope<'scope, 'run> {}

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Create a persistent owner backed by the same scheduler as this scope.
    ///
    /// Unlike [`Scope::child`], the returned owner is not tied to a callback
    /// stack frame. Its caller must dispose it when the owned object is
    /// removed; the DOM owner adapters use this as the row lifetime boundary.
    pub fn owned_scope(&self) -> OwnedScope<'scope, 'run> {
        let scheduler = self.frame.state.borrow().scheduler.clone();
        OwnedScope::new(scheduler)
    }

    /// Execute a child scope. All child nodes and computations are destroyed
    /// before this method returns, including during panic unwinding.
    pub fn child<R>(&self, f: impl for<'s1, 's2, 's3> FnOnce(&'s1 Scope<'s2, 's3>) -> R) -> R {
        let scheduler = self.frame.state.borrow().scheduler.clone();
        let frame = ScopeFrame::new(scheduler);
        let child = Scope {
            frame: &frame,
            _marker: PhantomData,
        };
        let result = catch_unwind(AssertUnwindSafe(|| f(&child)));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| frame.dispose()));
        match (result, dispose_result) {
            (Ok(value), Ok(())) => value,
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
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

    /// Register a type-erased callback under this scope.
    pub fn callback<F>(&self, f: F) -> Callback<'scope, 'run>
    where
        F: FnMut(AnyValue<'scope>) + 'scope,
    {
        let thunk = CallbackThunk::new(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与回调，
        // 因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_callback(thunk);
        Callback {
            handle: Handle::new(self.frame, raw),
        }
    }

    /// Create an effect owned by this scope and run it once immediately.
    pub fn effect<F>(&self, f: F) -> Effect<'scope, 'run>
    where
        F: FnMut() + 'scope,
    {
        let thunk = EffectThunk::new(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时（包括正常退出和 panic 恢复），`ScopeFrame::dispose` 会被强制调用
        // 并销毁所有节点与闭包，因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_effect(thunk);
        let handle = Handle::new(self.frame, raw);
        runtime::run_initial(&self.frame.state, raw);
        Effect { handle }
    }

    /// Register an effect and intentionally discard its diagnostic handle.
    pub fn watch<F>(&self, f: F)
    where
        F: FnMut() + 'scope,
    {
        let _ = self.effect(f);
    }

    /// Create a lazy memo whose dependents are notified only when its value
    /// changes according to `PartialEq`.
    pub fn memo<T, F>(&self, f: F) -> Memo<'scope, 'run, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        let thunk = MemoThunk::new::<T, F>(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与闭包，
        // 因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_memo(thunk, false);
        let handle = Handle::new(self.frame, raw);
        runtime::run_initial(&self.frame.state, raw);
        Memo {
            handle,
            marker: PhantomData,
        }
    }

    /// Create a lazy derived value without equality gating.
    pub fn derived<T, F>(&self, f: F) -> Derived<'scope, 'run, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        let thunk = MemoThunk::new_derived::<T, F>(f);
        // SAFETY: `thunk` 仅存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与闭包，
        // 因此将 `thunk` 的生命周期延伸至 `'run` 是 Sound 的。
        let thunk = unsafe { thunk.extend_lifetime() };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_memo(thunk, true);
        let handle = Handle::new(self.frame, raw);
        runtime::run_initial(&self.frame.state, raw);
        Derived {
            handle,
            marker: PhantomData,
        }
    }

    /// Create an empty host reference.
    pub fn node_ref<T: 'scope>(&self) -> NodeRef<'scope, 'run, T> {
        let value = AnyValue::new(Option::<T>::None);
        // SAFETY: `value` 存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与 node_ref 内部值，
        // 因此将 `value` 的生命周期延伸至 `'run` 是 Sound 的。
        let value = unsafe { transmute::<AnyValue<'scope>, AnyValue<'run>>(value) };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_node_ref(value);
        NodeRef {
            handle: Handle::new(self.frame, raw),
            marker: PhantomData,
        }
    }

    /// Create a signal owned by this scope.
    pub fn signal<T: 'scope>(
        &self,
        value: T,
    ) -> (ReadSignal<'scope, 'run, T>, WriteSignal<'scope, 'run, T>) {
        let value = AnyValue::new(value);
        // SAFETY: `value` 存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与 signal 内部值，
        // 因此将 `value` 的生命周期延伸至 `'run` 是 Sound 的。
        let value = unsafe { transmute::<AnyValue<'scope>, AnyValue<'run>>(value) };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_signal(value);
        let handle = Handle::new(self.frame, raw);
        (
            ReadSignal {
                handle,
                marker: PhantomData,
            },
            WriteSignal {
                handle,
                marker: PhantomData,
            },
        )
    }

    /// Create the paired form of a signal for callers that want one copyable
    /// capability instead of separate read/write values.
    pub fn rw_signal<T: 'scope>(&self, value: T) -> Signal<'scope, 'run, T> {
        let (read, write) = self.signal(value);
        Signal { read, write }
    }

    /// Store a non-reactive value under this scope.
    pub fn stored<T: 'scope>(&self, value: T) -> StoredValue<'scope, 'run, T> {
        let value = AnyValue::new(value);
        // SAFETY: `value` 存储在当前 `ScopeFrame`（词法生命周期为 `'scope`）对应的 `ScopeState` 中。
        // 当 `'scope` 作用域退出时，`ScopeFrame::dispose` 会被强制调用销毁所有节点与 stored value，
        // 因此将 `value` 的生命周期延伸至 `'run` 是 Sound 的。
        let value = unsafe { transmute::<AnyValue<'scope>, AnyValue<'run>>(value) };
        let raw = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_stored(value);
        StoredValue {
            handle: Handle::new(self.frame, raw),
            marker: PhantomData,
        }
    }
}

/// A persistent, owner-backed scope for a dynamic branch or list row.
///
/// The frame is heap allocated so its address remains stable while the owner
/// is stored by a controller. All callbacks registered through this type are
/// still bounded by the parent view lifetime and become inert after dispose.
pub struct OwnedScope<'scope, 'run> {
    frame: Box<ScopeFrame<'run>>,
    active: Cell<bool>,
    marker: PhantomData<fn(&'scope ())>,
}

impl<'scope, 'run> OwnedScope<'scope, 'run> {
    fn new(scheduler: Rc<RefCell<crate::runtime::GlobalScheduler>>) -> Self {
        Self {
            frame: Box::new(ScopeFrame::new(scheduler)),
            active: Cell::new(true),
            marker: PhantomData,
        }
    }

    pub(crate) fn new_for_scheduler(
        scheduler: Rc<RefCell<crate::runtime::GlobalScheduler>>,
    ) -> Self {
        Self::new(scheduler)
    }

    /// Create a nested persistent owner using the same scheduler.
    pub fn child(&self) -> Self {
        let scheduler = self.frame.state.borrow().scheduler.clone();
        let child = Self::new(scheduler);
        if !self.active.get() {
            child.dispose();
        }
        child
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    /// Register and immediately run an effect owned by this frame.
    pub fn effect<F>(&self, f: F)
    where
        F: FnMut() + 'scope,
    {
        if self.active.get() {
            self.with_scope(|scope| {
                let _ = scope.effect(f);
            });
        }
    }

    pub fn on_cleanup<F>(&self, f: F)
    where
        F: FnOnce() + 'scope,
    {
        if self.active.get() {
            self.with_scope(|scope| scope.on_cleanup(f));
        }
    }

    /// Dispose this owner exactly once. Cleanup panics follow the same
    /// propagation rules as lexical scope disposal.
    pub fn dispose(&self) {
        if !self.active.replace(false) {
            return;
        }
        self.frame.dispose();
    }

    fn with_scope<R>(&self, f: impl for<'row> FnOnce(&'row Scope<'row, 'run>) -> R) -> R {
        let scope = Scope {
            frame: &self.frame,
            _marker: PhantomData,
        };
        f(&scope)
    }
}

impl Drop for OwnedScope<'_, '_> {
    fn drop(&mut self) {
        self.dispose();
    }
}
