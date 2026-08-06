//! Lexical scope capabilities and lifetime boundaries.

use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

use super::node::{
    Callback, Derived, Effect, Memo, NodeRef, ReadSignal, RwSignal, StoredValue, WatchOptions,
    WriteSignal,
};
use crate::{
    ReactiveError, ReactiveResult,
    completion::{
        CompletionOnce, CompletionSender, create_completion_once, create_completion_sender,
    },
    handle::Handle,
    internal::value::{AnyValue, CallbackThunk, OnceThunk},
    runtime::{self, RuntimeInputs},
    scope::ScopeStorage,
};

/// A copyable capability to create and operate nodes in one lexical scope.
///
/// The scope itself does not own runtime state. The enclosing
/// [`ScopeStorage`] manages the lexical lifetime, which makes copying this
/// capability harmless and prevents a copied value from disposing the
/// original scope early.
///
/// Child node capabilities cannot be returned from the higher-ranked child
/// callback. The compile-fail case is covered by
/// `tests/ui/fail_child_handle_escape.rs`.
#[derive(Clone, Copy)]
pub struct Scope<'scope> {
    pub(crate) storage: &'scope ScopeStorage,
    pub(crate) _marker: PhantomData<fn() -> &'scope ()>,
}

impl<'scope> PartialEq for Scope<'scope> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.storage, other.storage)
    }
}

impl<'scope> Eq for Scope<'scope> {}

impl<'scope> Scope<'scope> {
    fn state(&self) -> Rc<RefCell<runtime::ScopeState<'scope>>> {
        // SAFETY: this capability is created only for the higher-ranked
        // lexical callback that owns the storage and disposes it before exit.
        unsafe { self.storage.typed_state() }
    }

    /// Create a persistent owner backed by the same scheduler as this scope.
    ///
    /// Unlike [`Scope::child`], the returned owner is not tied to a callback
    /// stack frame. Its caller must dispose it when the owned object is
    /// removed; the DOM owner adapters use this as the row lifetime boundary.
    pub fn owned_scope(&self) -> OwnedScope<'scope> {
        let state = self.state();
        let scheduler = state.borrow().scheduler.clone();
        OwnedScope::new(scheduler)
    }

    pub fn is_active(&self) -> bool {
        let state = self.state();
        let state = state.borrow();
        state
            .scheduler
            .borrow()
            .is_scope_active(self.storage.scope_id)
    }

    /// Validate opaque source provenance for a framework-owned registration.
    #[doc(hidden)]
    pub fn try_validate_inputs(&self, inputs: &RuntimeInputs) -> ReactiveResult<()> {
        runtime::validate_inputs(&self.state(), inputs)
    }

    /// Execute a child scope. All child nodes and computations are destroyed
    /// before this method returns, including during panic unwinding.
    pub fn child<R>(&self, f: impl for<'child> FnOnce(Scope<'child>) -> R) -> R {
        let state = self.state();
        let scheduler = state.borrow().scheduler.clone();
        let storage = ScopeStorage::new(scheduler.clone());
        let child = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let observer_frame = runtime::ObserverFrame::push_child(scheduler, storage.scope_id);
        let result = catch_unwind(AssertUnwindSafe(|| f(child)));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| storage.dispose_untracked()));
        drop(observer_frame);
        match (result, dispose_result) {
            (Ok(value), Ok(())) => value,
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
        }
    }

    /// Run a closure without recording signal dependencies. Ownership is
    /// unchanged because only the shared observer slot is modified.
    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> R {
        let state = self.state();
        runtime::with_untracked(&state, f)
    }

    /// Defer effect queue flushing until the outermost batch returns.
    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        let state = self.state();
        runtime::with_batch(&state, f)
    }

    /// Register cleanup on the current effect, or on this scope when no
    /// computation is active.
    pub fn on_cleanup<F>(&self, f: F)
    where
        F: FnOnce() + 'scope,
    {
        let thunk = OnceThunk::new(f);
        let state = self.state();
        let mut state = state
            .try_borrow_mut()
            .expect("ScopeState borrow failed during on_cleanup registration");
        state.register_cleanup(thunk);
    }

    /// Register a typed callback under this scope.
    pub fn callback<T, F>(&self, f: F) -> Callback<'scope, T>
    where
        T: 'scope,
        F: FnMut(T) + 'scope,
    {
        let thunk = CallbackThunk::new_typed(f);
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .expect("scope 在用户代码执行期间不应持有运行时借用")
            .create_callback(thunk)
            .unwrap_or_else(|error| panic!("创建 scoped callback 失败: {error}"));
        Callback {
            handle: Handle::new(self.storage, raw),
            marker: PhantomData,
        }
    }

    /// Create an effect owned by this scope and run it once immediately.
    pub fn effect<F>(&self, f: F) -> Effect<'scope>
    where
        F: FnMut() + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f)
    }

    /// Create an effect after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> Effect<'scope>
    where
        F: FnMut() + 'scope,
    {
        self.try_effect_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped effect 失败: {error}"))
    }

    /// Fallible computation creation boundary used by framework adapters.
    #[doc(hidden)]
    pub fn try_effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> ReactiveResult<Effect<'scope>>
    where
        F: FnMut() + 'scope,
    {
        let state = self.state();
        let raw = runtime::create_effect(&state, inputs, f)?;
        let handle = Handle::new(self.storage, raw);
        Ok(Effect { handle })
    }

    /// Create an effect that receives the value returned by its previous run.
    pub fn effect_with_previous<T, F>(&self, f: F) -> Effect<'scope>
    where
        T: 'scope,
        F: FnMut(Option<T>) -> T + 'scope,
    {
        self.effect_with_previous_from(RuntimeInputs::new(), f)
    }

    #[doc(hidden)]
    pub fn effect_with_previous_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> Effect<'scope>
    where
        T: 'scope,
        F: FnMut(Option<T>) -> T + 'scope,
    {
        self.try_effect_with_previous_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped previous effect 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_effect_with_previous_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
    ) -> ReactiveResult<Effect<'scope>>
    where
        T: 'scope,
        F: FnMut(Option<T>) -> T + 'scope,
    {
        let state = self.state();
        let raw = runtime::create_previous(&state, inputs, f)?;
        let handle = Handle::new(self.storage, raw);
        Ok(Effect { handle })
    }

    /// Create a getter-based watcher.
    pub fn watch_getter<T, G, C>(&self, getter: G, callback: C) -> Effect<'scope>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> T + 'scope,
        C: FnMut(&T, Option<&T>) + 'scope,
    {
        self.watch_getter_with_options(getter, callback, WatchOptions::default())
    }

    pub fn watch_getter_with_options<T, G, C>(
        &self,
        getter: G,
        callback: C,
        options: WatchOptions,
    ) -> Effect<'scope>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> T + 'scope,
        C: FnMut(&T, Option<&T>) + 'scope,
    {
        self.try_watch_getter_from(RuntimeInputs::new(), getter, callback, options)
            .unwrap_or_else(|error| panic!("创建 scoped watcher 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_watch_getter_from<T, G, C>(
        &self,
        inputs: RuntimeInputs,
        getter: G,
        callback: C,
        options: WatchOptions,
    ) -> ReactiveResult<Effect<'scope>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> T + 'scope,
        C: FnMut(&T, Option<&T>) + 'scope,
    {
        let state = self.state();
        let raw = runtime::create_watch(&state, inputs, getter, callback, options)?;
        let handle = Handle::new(self.storage, raw);
        Ok(Effect { handle })
    }

    /// Create a lazy memo whose dependents are notified only when its value
    /// changes according to `PartialEq`.
    pub fn memo<T, F>(&self, f: F) -> Memo<'scope, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.memo_from(RuntimeInputs::new(), f)
    }

    /// Create a memo after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn memo_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> Memo<'scope, T>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        self.try_memo_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped memo 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_memo_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
    ) -> ReactiveResult<Memo<'scope, T>>
    where
        T: PartialEq + 'scope,
        F: FnMut(Option<&T>) -> T + 'scope,
    {
        let state = self.state();
        let raw = runtime::create_memo(&state, inputs, f)?;
        let handle = Handle::new(self.storage, raw);
        Ok(Memo {
            handle,
            marker: PhantomData,
        })
    }

    /// Create a lazy derived value without equality gating.
    pub fn derived<T, F>(&self, f: F) -> Derived<'scope, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        self.derived_from(RuntimeInputs::new(), f)
    }

    /// Create a derived value after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn derived_from<T, F>(&self, inputs: RuntimeInputs, f: F) -> Derived<'scope, T>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        self.try_derived_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 scoped derived 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_derived_from<T, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
    ) -> ReactiveResult<Derived<'scope, T>>
    where
        T: 'scope,
        F: FnMut() -> T + 'scope,
    {
        let state = self.state();
        let raw = runtime::create_derived(&state, inputs, f)?;
        let handle = Handle::new(self.storage, raw);
        Ok(Derived {
            handle,
            marker: PhantomData,
        })
    }

    /// Create an empty host reference.
    pub fn node_ref<T: 'scope>(&self) -> NodeRef<'scope, T> {
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .expect("scope 在 node_ref 创建期间被借用")
            .create_node_ref(AnyValue::new(Option::<T>::None))
            .unwrap_or_else(|error| panic!("创建 scoped node_ref 失败: {error}"));
        NodeRef {
            handle: Handle::new(self.storage, raw),
            marker: PhantomData,
        }
    }

    /// Create a signal owned by this scope.
    pub fn signal<T: 'scope>(&self, value: T) -> (ReadSignal<'scope, T>, WriteSignal<'scope, T>) {
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .expect("scope 在 signal 创建期间被借用")
            .create_signal(AnyValue::new(value))
            .unwrap_or_else(|error| panic!("创建 scoped signal 失败: {error}"));
        let handle = Handle::new(self.storage, raw);
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
    pub fn rw_signal<T: 'scope>(&self, value: T) -> RwSignal<'scope, T> {
        let (read, write) = self.signal(value);
        RwSignal { read, write }
    }

    /// Store a non-reactive value under this scope.
    pub fn stored<T: 'scope>(&self, value: T) -> StoredValue<'scope, T> {
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .expect("scope 在 stored value 创建期间被借用")
            .create_stored(AnyValue::new(value))
            .unwrap_or_else(|error| panic!("创建 scoped stored value 失败: {error}"));
        StoredValue {
            handle: Handle::new(self.storage, raw),
            marker: PhantomData,
        }
    }

    /// Create a one-shot completion destination owned by this scope.
    pub fn completion_once<T: 'static, F>(&self, callback: F) -> CompletionOnce<T>
    where
        F: FnMut(T) + 'scope,
    {
        create_completion_once(self.storage, self.state(), callback)
    }

    /// Create a reusable completion destination owned by this scope.
    pub fn completion_sender<T: 'static, F>(&self, callback: F) -> CompletionSender<T>
    where
        F: FnMut(T) + 'scope,
    {
        create_completion_sender(self.storage, self.state(), callback)
    }
}

/// A persistent owner boundary for a dynamic branch or list row.
///
/// `OwnedScope` intentionally exposes owner operations only. Its storage is
/// owned by this value, so returning an ordinary node with the parent
/// `'scope` lifetime would let that node outlive the storage. Use a borrowed
/// [`Scope`] to create signals, memos, derived values, stored values,
/// callbacks, and node refs. An owned scope can register effects, cleanup, or
/// completion destinations whose handles remain borrowed from the owner.
pub struct OwnedScope<'scope> {
    storage: Box<ScopeStorage>,
    active: Cell<bool>,
    marker: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<'scope> OwnedScope<'scope> {
    fn state(&self) -> Rc<RefCell<runtime::ScopeState<'scope>>> {
        // SAFETY: OwnedScope's lifetime marker bounds every callback and
        // payload stored in this owner, and dispose runs before the owner can
        // be dropped.
        unsafe { self.storage.typed_state() }
    }

    fn new(scheduler: std::rc::Rc<std::cell::RefCell<crate::runtime::GlobalScheduler>>) -> Self {
        Self {
            storage: Box::new(ScopeStorage::new(scheduler)),
            active: Cell::new(true),
            marker: PhantomData,
        }
    }

    /// Create a nested persistent owner using the same scheduler.
    pub fn child(&self) -> Self {
        let state = self.state();
        let scheduler = state.borrow().scheduler.clone();
        let child = Self::new(scheduler);
        if !self.active.get() {
            child.dispose();
        }
        child
    }

    pub fn is_active(&self) -> bool {
        self.active.get()
    }

    #[doc(hidden)]
    pub fn try_validate_inputs(&self, inputs: &RuntimeInputs) -> ReactiveResult<()> {
        if !self.active.get() {
            return Err(ReactiveError::NoSuchNode);
        }
        runtime::validate_inputs(&self.state(), inputs)
    }

    /// Register and immediately run an effect owned by this frame.
    ///
    /// The returned effect handle borrows this owner and cannot outlive
    /// that borrow. The effect itself remains owned by `OwnedScope` until
    /// [`OwnedScope::dispose`] or `Drop`.
    pub fn effect<F>(&self, f: F) -> Effect<'_>
    where
        F: FnMut() + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f)
    }

    /// Register an effect after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> Effect<'_>
    where
        F: FnMut() + 'scope,
    {
        self.try_effect_from(inputs, f)
            .unwrap_or_else(|error| panic!("创建 owned effect 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_effect_from<F>(&self, inputs: RuntimeInputs, f: F) -> ReactiveResult<Effect<'_>>
    where
        F: FnMut() + 'scope,
    {
        if !self.active.get() {
            return Err(ReactiveError::NoSuchNode);
        }
        let state = self.state();
        let raw = runtime::create_effect(&state, inputs, f)?;
        let handle = Handle::new(&self.storage, raw);
        Ok(Effect { handle })
    }

    /// Create a getter-based watcher owned by this persistent scope.
    pub fn watch_getter<T, G, C>(&self, getter: G, callback: C) -> Effect<'_>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> T + 'scope,
        C: FnMut(&T, Option<&T>) + 'scope,
    {
        self.watch_getter_with_options(getter, callback, WatchOptions::default())
    }

    pub fn watch_getter_with_options<T, G, C>(
        &self,
        getter: G,
        callback: C,
        options: WatchOptions,
    ) -> Effect<'_>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> T + 'scope,
        C: FnMut(&T, Option<&T>) + 'scope,
    {
        self.try_watch_getter_from(RuntimeInputs::new(), getter, callback, options)
            .unwrap_or_else(|error| panic!("创建 owned watcher 失败: {error}"))
    }

    #[doc(hidden)]
    pub fn try_watch_getter_from<T, G, C>(
        &self,
        inputs: RuntimeInputs,
        getter: G,
        callback: C,
        options: WatchOptions,
    ) -> ReactiveResult<Effect<'_>>
    where
        T: PartialEq + 'scope,
        G: FnMut() -> T + 'scope,
        C: FnMut(&T, Option<&T>) + 'scope,
    {
        if !self.active.get() {
            return Err(ReactiveError::NoSuchNode);
        }
        let state = self.state();
        let raw = runtime::create_watch(&state, inputs, getter, callback, options)?;
        let handle = Handle::new(&self.storage, raw);
        Ok(Effect { handle })
    }

    pub fn on_cleanup<F>(&self, f: F)
    where
        F: FnOnce() + 'scope,
    {
        if !self.active.get() {
            return;
        }
        let state = self.state();
        let mut state = state
            .try_borrow_mut()
            .expect("owned scope 在 cleanup 注册期间被借用");
        state.register_cleanup(OnceThunk::new(f));
    }

    /// Create a one-shot completion destination owned by this persistent scope.
    pub fn completion_once<T: 'static, F>(&self, callback: F) -> CompletionOnce<T>
    where
        F: FnMut(T) + 'scope,
    {
        if !self.active.get() {
            return CompletionOnce::inactive();
        }
        create_completion_once(&self.storage, self.state(), callback)
    }

    /// Create a reusable completion destination owned by this persistent scope.
    pub fn completion_sender<T: 'static, F>(&self, callback: F) -> CompletionSender<T>
    where
        F: FnMut(T) + 'scope,
    {
        if !self.active.get() {
            return CompletionSender::inactive();
        }
        create_completion_sender(&self.storage, self.state(), callback)
    }

    /// Dispose this owner exactly once. Cleanup panics follow the same
    /// propagation rules as lexical scope disposal.
    pub fn dispose(&self) {
        if !self.active.replace(false) {
            return;
        }
        self.storage.dispose_untracked();
    }
}

impl Drop for OwnedScope<'_> {
    fn drop(&mut self) {
        self.dispose();
    }
}
