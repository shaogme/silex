//! Lexical scope capabilities and lifetime boundaries.

use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, UnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

use super::node::{
    Callback, Derived, Effect, Memo, NodeRef, ReadSignal, RwSignal, StoredValue, WatchOptions,
    WriteSignal,
};
use crate::{
    EffectInitError, EffectInitResult, ErrorHandler, ReactiveError, ReactiveResult,
    completion::{
        CompletionOnce, CompletionSender, create_completion_once, create_completion_sender,
    },
    error::ErrorHandlerEntry,
    handle::Handle,
    internal::value::{AnyValue, CallbackThunk, CleanupThunk},
    runtime::{self, RuntimeInputs},
    scope::ScopeStorage,
};

#[cfg(feature = "test-support")]
use crate::runtime::RuntimeSnapshot;

/// A copyable capability to create and operate nodes in one lexical scope.
///
/// The scope itself does not own runtime state. The enclosing
/// `ScopeStorage` manages the lexical lifetime, which makes copying this
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
        self.try_owned_scope()
            .unwrap_or_else(|error| panic!("创建 owned scope 失败: {error}"))
    }

    /// Create a persistent owner without converting an inactive or borrowed
    /// scope into a panic.
    pub fn try_owned_scope(&self) -> ReactiveResult<OwnedScope<'scope>> {
        let state = self.state();
        let state = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(OwnedScope::new(state.scheduler.clone()))
    }

    pub fn is_active(&self) -> bool {
        self.storage.is_active()
    }

    /// Register a callback owned by this scope and return a copyable handle.
    ///
    /// The callback remains in the scope registry until disposal. Creating a
    /// handler in a frequently rerun reactive callback therefore grows the
    /// registry until the owning scope ends.
    pub fn error_handler<E, F>(&self, handler: F) -> ErrorHandler<'scope, E>
    where
        E: 'scope,
        F: Fn(E) + 'scope,
    {
        self.try_error_handler(handler)
            .unwrap_or_else(|error| panic!("创建 scoped error handler 失败: {error}"))
    }

    pub fn try_error_handler<E, F>(&self, handler: F) -> ReactiveResult<ErrorHandler<'scope, E>>
    where
        E: 'scope,
        F: Fn(E) + 'scope,
    {
        let entry = ErrorHandlerEntry::new::<E, F>(handler);
        let state = self.state();
        let key = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .register_error_handler(entry)?;
        Ok(ErrorHandler::from_parts(self.storage, key))
    }

    /// Validate opaque source provenance for a framework-owned registration.
    #[doc(hidden)]
    pub fn try_validate_inputs(&self, inputs: &RuntimeInputs) -> ReactiveResult<()> {
        runtime::validate_inputs(&self.state(), inputs)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> RuntimeSnapshot {
        self.state().borrow().runtime_snapshot()
    }

    /// Execute a child scope. All child nodes and computations are destroyed
    /// before this method returns, including during panic unwinding.
    pub fn child<R>(&self, f: impl for<'child> FnOnce(Scope<'child>) -> R) -> R {
        assert!(
            self.storage.is_active(),
            "创建 child scope 失败: {}",
            ReactiveError::NoSuchNode
        );
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
    pub fn on_cleanup<E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> ReactiveResult<()>
    where
        E: 'scope,
        F: FnOnce() -> Result<(), E> + 'scope,
    {
        let thunk = CleanupThunk::new(f, error_handler);
        let state = self.state();
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        state.register_cleanup(thunk);
        Ok(())
    }

    /// Register a typed callback under this scope.
    pub fn callback<T, F>(&self, f: F) -> Callback<'scope, T>
    where
        T: 'scope,
        F: FnMut(T) + 'scope,
    {
        self.try_callback(f)
            .unwrap_or_else(|error| panic!("创建 scoped callback 失败: {error}"))
    }

    pub fn try_callback<T, F>(&self, f: F) -> ReactiveResult<Callback<'scope, T>>
    where
        T: 'scope,
        F: FnMut(T) + 'scope,
    {
        let thunk = CallbackThunk::new_typed(f);
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .create_callback(thunk)?;
        Ok(Callback {
            handle: Handle::new(self.storage, raw),
            marker: PhantomData,
        })
    }

    /// Create an effect owned by this scope and run it once immediately.
    pub fn effect<E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'scope>, E>
    where
        E: 'scope,
        F: FnMut() -> Result<(), E> + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f, error_handler)
    }

    /// Create an effect after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn effect_from<E, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'scope>, E>
    where
        E: 'scope,
        F: FnMut() -> Result<(), E> + 'scope,
    {
        let state = self.state();
        runtime::create_effect(&state, inputs, f, error_handler).map(|raw| Effect {
            handle: Handle::new(self.storage, raw),
        })
    }

    /// Create an effect that receives the value returned by its previous run.
    pub fn effect_with_previous<T, E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'scope>, E>
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        self.effect_with_previous_from(RuntimeInputs::new(), f, error_handler)
    }

    #[doc(hidden)]
    pub fn effect_with_previous_from<T, E, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'scope>, E>
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        let state = self.state();
        runtime::create_previous(&state, inputs, f, error_handler).map(|raw| Effect {
            handle: Handle::new(self.storage, raw),
        })
    }

    /// Create a getter-based watcher.
    pub fn watch_getter<T, E, G, C>(
        &self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'scope>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        self.watch_getter_with_options(getter, callback, error_handler, WatchOptions::default())
    }

    pub fn watch_getter_with_options<T, E, G, C>(
        &self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, E>,
        options: WatchOptions,
    ) -> EffectInitResult<Effect<'scope>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        self.watch_getter_from(
            RuntimeInputs::new(),
            getter,
            callback,
            error_handler,
            options,
        )
    }

    #[doc(hidden)]
    pub fn watch_getter_from<T, E, G, C>(
        &self,
        inputs: RuntimeInputs,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, E>,
        options: WatchOptions,
    ) -> EffectInitResult<Effect<'scope>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        let state = self.state();
        runtime::create_watch(&state, inputs, getter, callback, error_handler, options).map(|raw| {
            Effect {
                handle: Handle::new(self.storage, raw),
            }
        })
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
        self.try_node_ref()
            .unwrap_or_else(|error| panic!("创建 scoped node_ref 失败: {error}"))
    }

    pub fn try_node_ref<T: 'scope>(&self) -> ReactiveResult<NodeRef<'scope, T>> {
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .create_node_ref(AnyValue::new(Option::<T>::None))?;
        Ok(NodeRef {
            handle: Handle::new(self.storage, raw),
            marker: PhantomData,
        })
    }

    /// Create a signal owned by this scope.
    pub fn signal<T: 'scope>(&self, value: T) -> (ReadSignal<'scope, T>, WriteSignal<'scope, T>) {
        self.try_signal(value)
            .unwrap_or_else(|error| panic!("创建 scoped signal 失败: {error}"))
    }

    pub fn try_signal<T: 'scope>(
        &self,
        value: T,
    ) -> ReactiveResult<(ReadSignal<'scope, T>, WriteSignal<'scope, T>)> {
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .create_signal(AnyValue::new(value))?;
        let handle = Handle::new(self.storage, raw);
        Ok((
            ReadSignal {
                handle,
                marker: PhantomData,
            },
            WriteSignal {
                handle,
                marker: PhantomData,
            },
        ))
    }

    /// Create the paired form of a signal for callers that want one copyable
    /// capability instead of separate read/write values.
    pub fn rw_signal<T: 'scope>(&self, value: T) -> RwSignal<'scope, T> {
        let (read, write) = self.signal(value);
        RwSignal { read, write }
    }

    /// Store a non-reactive value under this scope.
    pub fn stored<T: 'scope>(&self, value: T) -> StoredValue<'scope, T> {
        self.try_stored(value)
            .unwrap_or_else(|error| panic!("创建 scoped stored value 失败: {error}"))
    }

    pub fn try_stored<T: 'scope>(&self, value: T) -> ReactiveResult<StoredValue<'scope, T>> {
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .create_stored(AnyValue::new(value))?;
        Ok(StoredValue {
            handle: Handle::new(self.storage, raw),
            marker: PhantomData,
        })
    }

    /// Create a one-shot completion destination owned by this scope.
    ///
    /// Use [`crate::unwind_safe`] when the callback captures interior-mutable
    /// state such as `Rc<RefCell<_>>`.
    pub fn completion_once<T: 'static, F>(&self, callback: F) -> CompletionOnce<T>
    where
        F: FnMut(T) + UnwindSafe + 'scope,
    {
        create_completion_once(self.storage, self.state(), callback)
    }

    /// Create a reusable completion destination owned by this scope.
    ///
    /// Use [`crate::unwind_safe`] when the callback captures interior-mutable
    /// state such as `Rc<RefCell<_>>`.
    pub fn completion_sender<T: 'static, F>(&self, callback: F) -> CompletionSender<T>
    where
        F: FnMut(T) + UnwindSafe + 'scope,
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
        self.try_child()
            .unwrap_or_else(|error| panic!("创建 owned child scope 失败: {error}"))
    }

    /// Create a nested persistent owner without panicking on an inactive
    /// owner or a conflicting scope borrow.
    pub fn try_child(&self) -> ReactiveResult<Self> {
        if !self.active.get() {
            return Err(ReactiveError::NoSuchNode);
        }
        let state = self.state();
        let state = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(Self::new(state.scheduler.clone()))
    }

    pub fn is_active(&self) -> bool {
        self.active.get() && self.storage.is_active()
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
    pub fn effect<E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'_>, E>
    where
        E: 'scope,
        F: FnMut() -> Result<(), E> + 'scope,
    {
        self.effect_from(RuntimeInputs::new(), f, error_handler)
    }

    /// Register an effect after validating all declared reactive inputs.
    #[doc(hidden)]
    pub fn effect_from<E, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'_>, E>
    where
        E: 'scope,
        F: FnMut() -> Result<(), E> + 'scope,
    {
        if !self.active.get() {
            return Err(EffectInitError::Registration(ReactiveError::NoSuchNode));
        }
        let state = self.state();
        runtime::create_effect(&state, inputs, f, error_handler).map(|raw| Effect {
            handle: Handle::new(&self.storage, raw),
        })
    }

    pub fn effect_with_previous<T, E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'_>, E>
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        self.effect_with_previous_from(RuntimeInputs::new(), f, error_handler)
    }

    #[doc(hidden)]
    pub fn effect_with_previous_from<T, E, F>(
        &self,
        inputs: RuntimeInputs,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'_>, E>
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        if !self.active.get() {
            return Err(EffectInitError::Registration(ReactiveError::NoSuchNode));
        }
        let state = self.state();
        runtime::create_previous(&state, inputs, f, error_handler).map(|raw| Effect {
            handle: Handle::new(&self.storage, raw),
        })
    }

    /// Create a getter-based watcher owned by this persistent scope.
    pub fn watch_getter<T, E, G, C>(
        &self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, E>,
    ) -> EffectInitResult<Effect<'_>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        self.watch_getter_with_options(getter, callback, error_handler, WatchOptions::default())
    }

    pub fn watch_getter_with_options<T, E, G, C>(
        &self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, E>,
        options: WatchOptions,
    ) -> EffectInitResult<Effect<'_>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        self.watch_getter_from(
            RuntimeInputs::new(),
            getter,
            callback,
            error_handler,
            options,
        )
    }

    #[doc(hidden)]
    pub fn watch_getter_from<T, E, G, C>(
        &self,
        inputs: RuntimeInputs,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, E>,
        options: WatchOptions,
    ) -> EffectInitResult<Effect<'_>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        if !self.active.get() {
            return Err(EffectInitError::Registration(ReactiveError::NoSuchNode));
        }
        let state = self.state();
        runtime::create_watch(&state, inputs, getter, callback, error_handler, options).map(|raw| {
            Effect {
                handle: Handle::new(&self.storage, raw),
            }
        })
    }

    pub fn on_cleanup<E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> ReactiveResult<()>
    where
        E: 'scope,
        F: FnOnce() -> Result<(), E> + 'scope,
    {
        if !self.active.get() {
            return Err(ReactiveError::NoSuchNode);
        }
        let cleanup = CleanupThunk::new(f, error_handler);
        let state = self.state();
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        state.register_cleanup(cleanup);
        Ok(())
    }

    /// Create a one-shot completion destination owned by this persistent scope.
    ///
    /// Use [`crate::unwind_safe`] when the callback captures interior-mutable
    /// state such as `Rc<RefCell<_>>`.
    pub fn completion_once<T: 'static, F>(&self, callback: F) -> CompletionOnce<T>
    where
        F: FnMut(T) + UnwindSafe + 'scope,
    {
        if !self.is_active() {
            return CompletionOnce::inactive();
        }
        create_completion_once(&self.storage, self.state(), callback)
    }

    /// Create a reusable completion destination owned by this persistent scope.
    ///
    /// Use [`crate::unwind_safe`] when the callback captures interior-mutable
    /// state such as `Rc<RefCell<_>>`.
    pub fn completion_sender<T: 'static, F>(&self, callback: F) -> CompletionSender<T>
    where
        F: FnMut(T) + UnwindSafe + 'scope,
    {
        if !self.is_active() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{GlobalScheduler, ScopeState};
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Snapshot {
        nodes: usize,
        data: usize,
        edges: usize,
        roots: usize,
        queue: usize,
    }

    fn snapshot<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>) -> Snapshot {
        let state_ref = state.borrow();
        let queue = state_ref.scheduler.borrow().global_queue.len();
        Snapshot {
            nodes: state_ref.nodes.len(),
            data: state_ref.data.len(),
            edges: state_ref.edges.len(),
            roots: state_ref.roots.len(),
            queue,
        }
    }

    fn rejected_creations(scope: Scope<'_>) -> Vec<ReactiveResult<()>> {
        vec![
            scope.try_signal(0_i32).map(|_| ()),
            scope.try_stored(()).map(|_| ()),
            scope.try_callback(|_: ()| {}).map(|_| ()),
            scope.try_node_ref::<()>().map(|_| ()),
        ]
    }

    fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
        scope.error_handler(|_| {})
    }

    #[test]
    fn inactive_cleanup_rejects_all_value_creation_without_metadata_changes() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let state = scope.state();
        let state_in_cleanup = state.clone();
        let observed = Rc::new(RefCell::new(None));
        let observed_in_cleanup = observed.clone();
        let scope_in_cleanup = scope;
        scope
            .on_cleanup(
                move || {
                    assert_eq!(
                        rejected_creations(scope_in_cleanup),
                        vec![
                            Err(ReactiveError::NoSuchNode),
                            Err(ReactiveError::NoSuchNode),
                            Err(ReactiveError::NoSuchNode),
                            Err(ReactiveError::NoSuchNode),
                        ]
                    );
                    *observed_in_cleanup.borrow_mut() = Some(snapshot(&state_in_cleanup));
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");

        let before_dispose = snapshot(&state);
        storage.dispose_untracked();

        assert_eq!(*observed.borrow(), Some(before_dispose));
        assert_eq!(
            snapshot(&state),
            Snapshot {
                nodes: 0,
                data: 0,
                edges: 0,
                roots: 0,
                queue: 0,
            }
        );
    }

    struct DropProbe<'scope> {
        scope: Scope<'scope>,
        state: Rc<RefCell<ScopeState<'scope>>>,
        expected: Snapshot,
        observations: Observations,
    }

    type Observations = Rc<RefCell<Vec<(Snapshot, Vec<ReactiveResult<()>>)>>>;

    impl Drop for DropProbe<'_> {
        fn drop(&mut self) {
            let result = rejected_creations(self.scope);
            let actual = snapshot(&self.state);
            assert_eq!(actual, self.expected);
            self.observations.borrow_mut().push((actual, result));
        }
    }

    fn drop_probe<'scope>(
        scope: Scope<'scope>,
        state: Rc<RefCell<ScopeState<'scope>>>,
        expected: Snapshot,
        observations: Observations,
    ) -> DropProbe<'scope> {
        DropProbe {
            scope,
            state,
            expected,
            observations,
        }
    }

    #[test]
    fn inactive_payload_drop_rejects_creation_after_node_removal() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let state = scope.state();
        let observations = Rc::new(RefCell::new(Vec::new()));

        let _sentinel = scope.signal(());
        let callback_probe = drop_probe(
            scope,
            state.clone(),
            Snapshot {
                nodes: 2,
                data: 2,
                edges: 0,
                roots: 2,
                queue: 0,
            },
            observations.clone(),
        );
        let _callback = scope.callback(move |_: ()| {
            let _ = &callback_probe;
        });

        let stored_probe = drop_probe(
            scope,
            state.clone(),
            Snapshot {
                nodes: 1,
                data: 1,
                edges: 0,
                roots: 1,
                queue: 0,
            },
            observations.clone(),
        );
        let _stored = scope.stored(stored_probe);

        let node_ref = scope.node_ref::<DropProbe<'_>>();
        node_ref
            .set(drop_probe(
                scope,
                state,
                Snapshot {
                    nodes: 0,
                    data: 0,
                    edges: 0,
                    roots: 0,
                    queue: 0,
                },
                observations.clone(),
            ))
            .expect("node ref should accept the probe while active");

        storage.dispose_untracked();

        let observations = observations.borrow();
        assert_eq!(observations.len(), 3);
        for (_, result) in observations.iter() {
            assert!(
                result
                    .iter()
                    .all(|value| *value == Err(ReactiveError::NoSuchNode))
            );
        }
        assert_eq!(
            observations[0].0,
            Snapshot {
                nodes: 2,
                data: 2,
                edges: 0,
                roots: 2,
                queue: 0,
            }
        );
        assert_eq!(
            observations[1].0,
            Snapshot {
                nodes: 1,
                data: 1,
                edges: 0,
                roots: 1,
                queue: 0,
            }
        );
        assert_eq!(
            observations[2].0,
            Snapshot {
                nodes: 0,
                data: 0,
                edges: 0,
                roots: 0,
                queue: 0,
            }
        );
    }

    #[test]
    fn inactive_scope_rejects_new_child_and_owned_scopes() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let scope_copy = scope;
        let child_rejected = Rc::new(RefCell::new(false));
        let owned_rejected = Rc::new(RefCell::new(false));
        let child_rejected_in_cleanup = child_rejected.clone();
        let owned_rejected_in_cleanup = owned_rejected.clone();
        scope
            .on_cleanup(
                move || {
                    *child_rejected_in_cleanup.borrow_mut() =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            scope_copy.child(|_| ())
                        }))
                        .is_err();
                    *owned_rejected_in_cleanup.borrow_mut() =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            scope_copy.owned_scope()
                        }))
                        .is_err();
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");

        storage.dispose_untracked();

        assert!(*child_rejected.borrow());
        assert!(*owned_rejected.borrow());
    }

    #[test]
    fn disposed_scope_cannot_register_after_scope_id_reuse() {
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler.clone());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        storage.dispose_untracked();

        let replacement = ScopeStorage::new(scheduler);
        assert!(matches!(
            scope.try_signal(0_i32),
            Err(ReactiveError::NoSuchNode)
        ));
        assert!(replacement.is_active());
        assert_eq!(replacement.state.borrow().nodes.len(), 0);

        replacement.dispose_untracked();
    }

    #[test]
    fn owned_registration_preserves_borrow_conflict() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let owner = scope.owned_scope();
        let state = owner.state();
        let state_borrow = state.borrow_mut();

        assert!(matches!(
            owner.effect(|| Ok(()), handler(scope)),
            Err(EffectInitError::Registration(ReactiveError::BorrowConflict))
        ));

        drop(state_borrow);
        owner.dispose();
        storage.dispose_untracked();
    }
}
