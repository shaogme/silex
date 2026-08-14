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
    CleanupError, ComputationInitError, ComputationInitResult, ErrorHandler, ReactiveError,
    ReactiveResult,
    completion::{
        CompletionOnce, CompletionSender, create_completion_once, create_completion_sender,
    },
    error::ErrorHandlerEntry,
    handle::Handle,
    runtime,
    runtime::storage::{CallbackThunk, CleanupThunk},
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
        self.storage.owner_token(PhantomData).state()
    }

    /// Create a persistent owner backed by the same scheduler as this scope.
    ///
    /// Unlike [`Scope::child`], the returned owner is not tied to a callback
    /// stack frame. Its caller must dispose it when the owned object is
    /// removed; the DOM owner adapters use this as the row lifetime boundary.
    pub fn owned_scope(&self) -> ReactiveResult<OwnedScope<'scope>> {
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

    /// Check whether another scope belongs to this runtime scheduler.
    pub fn same_runtime(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.storage.scheduler(), &other.storage.scheduler())
    }

    /// Register a callback owned by this scope and return a copyable handle.
    ///
    /// The callback remains in the scope registry until disposal. Creating a
    /// handler in a frequently rerun reactive callback therefore grows the
    /// registry until the owning scope ends.
    pub fn error_handler<E, F>(&self, handler: F) -> ReactiveResult<ErrorHandler<'scope, E>>
    where
        E: 'scope,
        F: Fn(E) + 'scope,
    {
        let state = self.state();
        let callback = self.storage.alloc_handler(handler);
        let key = match state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| {
                state.register_error_handler(ErrorHandlerEntry { owner: callback })
            }) {
            Ok(key) => key,
            Err(error) => {
                callback.clear();
                return Err(error);
            }
        };
        Ok(ErrorHandler::from_parts(self.storage, key, callback))
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> RuntimeSnapshot {
        self.state().borrow().runtime_snapshot()
    }

    /// Execute a child scope. The parent computation remains the active
    /// observer while the callback runs, while child-local transient sources
    /// are blocked from escaping the callback. All child nodes and
    /// computations are destroyed before this method returns, including
    /// during panic unwinding.
    pub fn child<R>(&self, f: impl for<'child> FnOnce(Scope<'child>) -> R) -> ReactiveResult<R> {
        if !self.storage.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
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
            (Ok(value), Ok(())) => Ok(value),
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
        }
    }

    /// Run a closure without recording signal dependencies. Ownership is
    /// unchanged because both observer slots are temporarily cleared.
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
    /// computation is active. Cleanup reads are untracked, and tracked reads
    /// are also disabled while cleanup runs.
    ///
    /// During final disposal of this scope, the cleanup runs while nodes and
    /// payloads still exist. It may synchronously read or update a
    /// [`StoredValue`](super::node::StoredValue) from this same scope even
    /// though the ordinary scope capability is already inactive. This window
    /// applies only to final scope disposal, not to an effect rerun or a
    /// single-node stop; other scope APIs remain unavailable.
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
    pub fn callback<T, E, F>(&self, f: F) -> ReactiveResult<Callback<'scope, T, E>>
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(T) -> Result<(), E> + 'scope,
    {
        let thunk = self.storage.alloc_slot(CallbackThunk::new(f));
        let state = self.state();
        let raw = match state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_callback(thunk))
        {
            Ok(raw) => raw,
            Err(error) => {
                thunk.slot().clear();
                return Err(error);
            }
        };
        Ok(Callback {
            handle: Handle::new(self.storage, raw),
            callback: thunk,
            marker: PhantomData,
        })
    }

    /// Create an effect owned by this scope and run it once immediately.
    pub fn effect<E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> ComputationInitResult<Effect<'scope>, E>
    where
        E: 'scope,
        F: FnMut() -> Result<(), E> + 'scope,
    {
        let state = self.state();
        runtime::create_effect(self.storage, &state, f, error_handler).map(|raw| Effect {
            handle: Handle::new(self.storage, raw),
        })
    }

    /// Create an effect that receives the value returned by its previous run.
    pub fn effect_with_previous<T, E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> ComputationInitResult<Effect<'scope>, E>
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        let state = self.state();
        runtime::create_previous(self.storage, &state, f, error_handler).map(|raw| Effect {
            handle: Handle::new(self.storage, raw),
        })
    }

    /// Create a getter-based watcher.
    pub fn watch_getter<T, E, G, C>(
        &self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'scope, E>,
    ) -> ComputationInitResult<Effect<'scope>, E>
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
    ) -> ComputationInitResult<Effect<'scope>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        let state = self.state();
        runtime::create_watch(
            self.storage,
            &state,
            getter,
            callback,
            error_handler,
            options,
        )
        .map(|raw| Effect {
            handle: Handle::new(self.storage, raw),
        })
    }

    /// Create a lazy memo whose dependents are notified only when its value
    /// changes according to `PartialEq`.
    pub fn memo<T, E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> ComputationInitResult<Memo<'scope, T, E>, E>
    where
        T: PartialEq + 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        let state = self.state();
        let created = runtime::create_memo(self.storage, &state, f, error_handler)?;
        let handle = Handle::new(self.storage, created.raw);
        Ok(Memo {
            handle,
            value: created.value,
            errors: created.errors,
            marker: PhantomData,
        })
    }

    /// Create a lazy derived value without equality gating.
    pub fn derived<T, E, F>(
        &self,
        f: F,
        error_handler: ErrorHandler<'scope, E>,
    ) -> ComputationInitResult<Derived<'scope, T, E>, E>
    where
        T: 'scope,
        E: 'scope,
        F: FnMut() -> Result<T, E> + 'scope,
    {
        let state = self.state();
        let created = runtime::create_derived(self.storage, &state, f, error_handler)?;
        let handle = Handle::new(self.storage, created.raw);
        Ok(Derived {
            handle,
            value: created.value,
            errors: created.errors,
            marker: PhantomData,
        })
    }

    /// Create an empty host reference.
    pub fn node_ref<T: 'scope>(&self) -> ReactiveResult<NodeRef<'scope, T>> {
        let slot = self.storage.alloc_slot(None::<T>);
        let state = self.state();
        let raw = match state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_node_ref(slot))
        {
            Ok(raw) => raw,
            Err(error) => {
                slot.slot().clear();
                return Err(error);
            }
        };
        Ok(NodeRef {
            handle: Handle::new(self.storage, raw),
            value: slot,
            marker: PhantomData,
        })
    }

    /// Create a signal owned by this scope.
    pub fn signal<T: 'scope>(
        &self,
        value: T,
    ) -> ReactiveResult<(ReadSignal<'scope, T>, WriteSignal<'scope, T>)> {
        let slot = self.storage.alloc_slot(value);
        let state = self.state();
        let raw = match state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_signal(slot))
        {
            Ok(raw) => raw,
            Err(error) => {
                slot.slot().clear();
                return Err(error);
            }
        };
        let handle = Handle::new(self.storage, raw);
        Ok((
            ReadSignal {
                handle,
                value: slot,
                marker: PhantomData,
            },
            WriteSignal {
                handle,
                value: slot,
                marker: PhantomData,
            },
        ))
    }

    /// Create the paired form of a signal for callers that want one copyable
    /// capability instead of separate read/write values.
    pub fn rw_signal<T: 'scope>(&self, value: T) -> ReactiveResult<RwSignal<'scope, T>> {
        let (read, write) = self.signal(value)?;
        Ok(RwSignal { read, write })
    }

    /// Store a non-reactive value under this scope.
    pub fn stored<T: 'scope>(&self, value: T) -> ReactiveResult<StoredValue<'scope, T>> {
        let slot = self.storage.alloc_slot(value);
        let state = self.state();
        let raw = match state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_stored(slot))
        {
            Ok(raw) => raw,
            Err(error) => {
                slot.slot().clear();
                return Err(error);
            }
        };
        Ok(StoredValue {
            handle: Handle::new(self.storage, raw),
            value: slot,
            marker: PhantomData,
        })
    }

    /// Create a one-shot completion destination owned by this scope.
    ///
    /// Use [`crate::unwind_safe`] when the callback captures interior-mutable
    /// state such as `Rc<RefCell<_>>`.
    pub fn completion_once<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionOnce<T, E>>
    where
        E: 'scope,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'scope,
    {
        create_completion_once(self.storage, self.state(), callback)
    }

    /// Create a reusable completion destination owned by this scope.
    ///
    /// Use [`crate::unwind_safe`] when the callback captures interior-mutable
    /// state such as `Rc<RefCell<_>>`.
    pub fn completion_sender<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionSender<T, E>>
    where
        E: 'scope,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'scope,
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
    fn state<'owner>(&'owner self) -> Rc<RefCell<runtime::ScopeState<'owner>>> {
        self.storage.owner_token(PhantomData).state()
    }

    fn new(scheduler: std::rc::Rc<std::cell::RefCell<crate::runtime::GlobalScheduler>>) -> Self {
        Self {
            storage: Box::new(ScopeStorage::new(scheduler)),
            active: Cell::new(true),
            marker: PhantomData,
        }
    }

    /// Create a nested persistent owner using the same scheduler.
    pub fn child(&self) -> ReactiveResult<Self> {
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

    /// Register and immediately run an effect owned by this frame.
    ///
    /// The returned effect handle borrows this owner and cannot outlive
    /// that borrow. The effect itself remains owned by `OwnedScope` until
    /// [`OwnedScope::dispose`] or `Drop`.
    pub fn effect<'owner, E, F>(
        &'owner self,
        f: F,
        error_handler: ErrorHandler<'owner, E>,
    ) -> ComputationInitResult<Effect<'owner>, E>
    where
        E: 'owner,
        F: FnMut() -> Result<(), E> + 'owner,
    {
        if !self.active.get() {
            return Err(ComputationInitError::Registration(
                ReactiveError::NoSuchNode,
            ));
        }
        let state = self.state();
        runtime::create_effect(&self.storage, &state, f, error_handler).map(|raw| Effect {
            handle: Handle::new(&self.storage, raw),
        })
    }

    pub fn effect_with_previous<'owner, T, E, F>(
        &'owner self,
        f: F,
        error_handler: ErrorHandler<'owner, E>,
    ) -> ComputationInitResult<Effect<'owner>, E>
    where
        T: 'owner,
        E: 'owner,
        F: FnMut(Option<&T>) -> Result<T, E> + 'owner,
    {
        if !self.active.get() {
            return Err(ComputationInitError::Registration(
                ReactiveError::NoSuchNode,
            ));
        }
        let state = self.state();
        runtime::create_previous(&self.storage, &state, f, error_handler).map(|raw| Effect {
            handle: Handle::new(&self.storage, raw),
        })
    }

    /// Create a getter-based watcher owned by this persistent scope.
    pub fn watch_getter<'owner, T, E, G, C>(
        &'owner self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'owner, E>,
    ) -> ComputationInitResult<Effect<'owner>, E>
    where
        T: PartialEq + 'owner,
        E: 'owner,
        G: FnMut() -> Result<T, E> + 'owner,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'owner,
    {
        self.watch_getter_with_options(getter, callback, error_handler, WatchOptions::default())
    }

    pub fn watch_getter_with_options<'owner, T, E, G, C>(
        &'owner self,
        getter: G,
        callback: C,
        error_handler: ErrorHandler<'owner, E>,
        options: WatchOptions,
    ) -> ComputationInitResult<Effect<'owner>, E>
    where
        T: PartialEq + 'owner,
        E: 'owner,
        G: FnMut() -> Result<T, E> + 'owner,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'owner,
    {
        if !self.active.get() {
            return Err(ComputationInitError::Registration(
                ReactiveError::NoSuchNode,
            ));
        }
        let state = self.state();
        runtime::create_watch(
            &self.storage,
            &state,
            getter,
            callback,
            error_handler,
            options,
        )
        .map(|raw| Effect {
            handle: Handle::new(&self.storage, raw),
        })
    }

    /// Register cleanup for this persistent owner.
    ///
    /// During final disposal of the owner, the cleanup runs before its nodes
    /// and payloads are dropped. A `StoredValue` belonging to the same storage
    /// remains synchronously accessible in that window, while the owner is
    /// still inactive for all ordinary APIs. This guarantee is limited to
    /// final owner disposal and does not apply to effect reruns or node stops.
    pub fn on_cleanup<'owner, E, F>(
        &'owner self,
        f: F,
        error_handler: ErrorHandler<'owner, E>,
    ) -> ReactiveResult<()>
    where
        E: 'owner,
        F: FnOnce() -> Result<(), E> + 'owner,
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
    pub fn completion_once<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionOnce<T, E>>
    where
        E: 'scope,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'scope,
    {
        if !self.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        create_completion_once(&self.storage, self.state(), callback)
    }

    /// Create a reusable completion destination owned by this persistent scope.
    ///
    /// Use [`crate::unwind_safe`] when the callback captures interior-mutable
    /// state such as `Rc<RefCell<_>>`.
    pub fn completion_sender<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionSender<T, E>>
    where
        E: 'scope,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'scope,
    {
        if !self.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        create_completion_sender(&self.storage, self.state(), callback)
    }

    /// Dispose this owner exactly once. Cleanup panics follow the same
    /// propagation rules as lexical scope disposal.
    pub fn dispose(&self) -> Result<(), CleanupError> {
        if !self.active.replace(false) {
            return Ok(());
        }
        match catch_unwind(AssertUnwindSafe(|| self.storage.dispose_untracked())) {
            Ok(()) => Ok(()),
            Err(panic) => Err(CleanupError::from_panic(panic)),
        }
    }
}

impl Drop for OwnedScope<'_> {
    fn drop(&mut self) {
        let _ = self.dispose();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallbackInvokeError,
        runtime::{GlobalScheduler, ScopeState},
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Snapshot {
        nodes: usize,
        data: usize,
        edges: usize,
        roots: usize,
        queue: usize,
    }

    struct ImmediateDropProbe(Rc<Cell<usize>>);

    impl Drop for ImmediateDropProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
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
            scope.signal(0_i32).map(|_| ()),
            scope.stored(()).map(|_| ()),
            scope.callback(|_: ()| Ok::<(), ()>(())).map(|_| ()),
            scope.node_ref::<()>().map(|_| ()),
        ]
    }

    fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
        scope.error_handler(|_| {}).expect("handler registration")
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

    #[test]
    fn rejected_value_creation_drops_allocated_payload_immediately() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let dropped = Rc::new(Cell::new(0));
        let dropped_in_cleanup = dropped.clone();
        let scope_in_cleanup = scope;
        scope
            .on_cleanup(
                move || {
                    let signal =
                        scope_in_cleanup.signal(ImmediateDropProbe(dropped_in_cleanup.clone()));
                    assert!(matches!(signal, Err(ReactiveError::NoSuchNode)));
                    assert_eq!(dropped_in_cleanup.get(), 1);

                    let stored =
                        scope_in_cleanup.stored(ImmediateDropProbe(dropped_in_cleanup.clone()));
                    assert!(matches!(stored, Err(ReactiveError::NoSuchNode)));
                    assert_eq!(dropped_in_cleanup.get(), 2);

                    let callback_probe = ImmediateDropProbe(dropped_in_cleanup.clone());
                    let callback = scope_in_cleanup.callback(move |_: ()| {
                        let _probe = &callback_probe;
                        Ok::<(), ()>(())
                    });
                    assert!(matches!(callback, Err(ReactiveError::NoSuchNode)));
                    assert_eq!(dropped_in_cleanup.get(), 3);
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");

        storage.dispose_untracked();
        assert_eq!(dropped.get(), 3);
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

        let _sentinel = scope.signal(()).expect("fallible reactive creation");
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
        let _callback = scope
            .callback(move |_: ()| {
                let _ = &callback_probe;
                Ok::<(), ()>(())
            })
            .expect("callback should register");

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
        let _stored = scope
            .stored(stored_probe)
            .expect("fallible reactive creation");

        let node_ref = scope
            .node_ref::<DropProbe<'_>>()
            .expect("node ref creation");
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
                    *child_rejected_in_cleanup.borrow_mut() = scope_copy.child(|_| ()).is_err();
                    *owned_rejected_in_cleanup.borrow_mut() = scope_copy.owned_scope().is_err();
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
    fn cleanup_scope_allocation_does_not_reuse_the_scope_being_disposed() {
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler.clone());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let disposing_id = storage.scope_id;
        let allocated_id = Rc::new(Cell::new(None));
        let allocated_id_in_cleanup = allocated_id.clone();
        let scheduler_in_cleanup = scheduler.clone();
        scope
            .on_cleanup(
                move || {
                    let replacement = ScopeStorage::new(scheduler_in_cleanup.clone());
                    allocated_id_in_cleanup.set(Some(replacement.scope_id));
                    assert_ne!(replacement.scope_id, disposing_id);
                    replacement.dispose_untracked();
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");

        storage.dispose_untracked();

        assert_ne!(allocated_id.get(), Some(disposing_id));
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
            scope.signal(0_i32),
            Err(ReactiveError::NoSuchNode)
        ));
        assert!(replacement.is_active());
        assert_eq!(replacement.state.borrow().nodes.len(), 0);

        replacement.dispose_untracked();
    }

    #[test]
    fn disposed_callback_returns_a_runtime_error() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let callback = scope
            .callback(|_: ()| Ok::<(), ()>(()))
            .expect("callback should register");

        storage.dispose_untracked();

        assert!(matches!(
            callback.invoke(()),
            Err(CallbackInvokeError::Runtime(ReactiveError::NoSuchNode))
        ));
    }

    #[test]
    fn owned_registration_preserves_borrow_conflict() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = Scope {
            storage: &storage,
            _marker: PhantomData,
        };
        let owner = scope.owned_scope().expect("fallible reactive creation");
        let state = owner.state();
        let state_borrow = state.borrow_mut();

        assert!(matches!(
            owner.effect(|| Ok(()), handler(scope)),
            Err(ComputationInitError::Registration(
                ReactiveError::BorrowConflict
            ))
        ));

        drop(state_borrow);
        owner.dispose().expect("owner disposal");
        storage.dispose_untracked();
    }
}
