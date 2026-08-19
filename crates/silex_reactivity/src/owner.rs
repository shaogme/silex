//! Unified owner handles and lifetime-bearing owner access.
//!
//! `OwnerHandle` owns the runtime close operation. `OwnerAccess` is a borrowed
//! typed view and is the only new API entry point that creates scope-local
//! payloads. Runtime identity and generation are checked by the existing
//! storage/scheduler boundary; the Rust lifetime remains carried by this
//! borrowed view and is never reconstructed from an owner id alone.

mod node;
mod storage;

pub(crate) use storage::ScopeStorage;

use crate::{
    ComputationInitResult, ErrorHandlerInput, ErrorHandlerToken, ReactiveError, ReactiveResult,
    completion::{
        CompletionOnce, CompletionSender, create_completion_once, create_completion_sender,
    },
    error::{ErrorHandlerEntry, HandlerOwner, HandlerRecord},
    handle::Handle,
    root::{
        CloseError, ClosePhase, CloseSource, CloseTransaction, TransientScopeError,
        TransientScopeResult,
    },
    runtime::storage::{CallbackThunk, CleanupThunk},
    runtime::{self, OwnerMode},
    unsafe_boundary::{WeakOwnerToken, persistent_child_storage},
};
pub use node::{
    Callback, Computed, EffectHandle, NodeRef, ReadSignal, RwSignal, StoredValue, WatchOptions,
    WriteSignal,
};
use std::{cell::Cell, future::Future, marker::PhantomData, panic::UnwindSafe, pin::Pin, rc::Rc};

/// A persistent or root owner with explicit close authority.
pub struct OwnerHandle {
    pub(crate) storage: Rc<ScopeStorage>,
    runtime_slot: Option<Rc<Cell<bool>>>,
    closed: Cell<bool>,
}

impl OwnerHandle {
    pub(crate) fn new(storage: Rc<ScopeStorage>, runtime_slot: Option<Rc<Cell<bool>>>) -> Self {
        Self {
            storage,
            runtime_slot,
            closed: Cell::new(false),
        }
    }

    fn new_child(storage: &ScopeStorage) -> ReactiveResult<Self> {
        if !storage.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        let state = storage.owner_token().state();
        let scheduler = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .scheduler
            .clone();
        let child =
            ScopeStorage::new_with_owner(scheduler, Some(storage.owner_id), OwnerMode::Persistent);
        let child = Rc::new(child);
        storage.children.borrow_mut().push(child.clone());
        Ok(Self {
            storage: child,
            runtime_slot: None,
            closed: Cell::new(false),
        })
    }

    /// Borrow the typed node-creation and operation view for this owner.
    pub fn access(&self) -> OwnerAccess<'_> {
        OwnerAccess {
            storage: self.storage.as_ref(),
            marker: PhantomData,
        }
    }

    pub fn with_access<R>(&self, f: impl FnOnce(OwnerAccess<'_>) -> R) -> R {
        f(self.access())
    }

    /// Run a future while retaining the borrowed owner capability.
    ///
    /// The boxed future carries the same lifetime as the owner access. This
    /// keeps async work lexically tied to the owner without manufacturing a
    /// `'static` capability or weakening the close boundary.
    pub async fn with_access_async<R>(
        &self,
        f: impl for<'owner> FnOnce(OwnerAccess<'owner>) -> Pin<Box<dyn Future<Output = R> + 'owner>>,
    ) -> R {
        f(self.access()).await
    }

    /// Create a persistent child in the same runtime owner registry.
    pub fn create_child(&self) -> ReactiveResult<Self> {
        Self::new_child(self.storage.as_ref())
    }

    /// Close this owner. A failed close leaves the owner in its runtime
    /// closing phase so the caller can retry.
    pub fn close(&self) -> Result<(), CloseError> {
        if self.closed.get() {
            return Ok(());
        }
        let result = close_owner_tree(&self.storage).map_or(Ok(()), Err);
        if result.is_ok() {
            self.closed.set(true);
            if let Some(runtime_slot) = &self.runtime_slot {
                runtime_slot.set(false);
            }
        }
        result
    }

    pub fn is_active(&self) -> bool {
        !self.closed.get() && self.storage.is_active()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> runtime::RuntimeSnapshot {
        self.access().runtime_snapshot()
    }
}

impl Drop for OwnerHandle {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Hidden adapter that keeps a persistent child access and its close authority
/// together.
///
/// The adapter is branded by the lifetime of the parent [`OwnerAccess`]. Its
/// child access is retained by the parent's child-storage allocation, while
/// runtime operations still reject the access after close. Closing the adapter
/// is idempotent even when an ancestor has already recursively closed the
/// child.
#[doc(hidden)]
pub struct PersistentOwnerAccess<'parent> {
    handle: OwnerHandle,
    access: OwnerAccess<'parent>,
    marker: PhantomData<&'parent ()>,
}

impl<'parent> PersistentOwnerAccess<'parent> {
    fn from_handle(parent: &'parent ScopeStorage, handle: OwnerHandle) -> ReactiveResult<Self> {
        let storage = persistent_child_storage(parent, &handle.storage)?;
        Ok(Self {
            handle,
            access: OwnerAccess {
                storage,
                marker: PhantomData,
            },
            marker: PhantomData,
        })
    }

    /// Borrow the typed access for this persistent child.
    #[doc(hidden)]
    pub fn access(&self) -> OwnerAccess<'parent> {
        self.access
    }

    /// Close the child exactly once from the caller's perspective.
    ///
    /// If a parent owner already closed the child, the runtime's released
    /// phase makes this a successful no-op. Other close failures retain their
    /// original [`CloseError`] classification for a caller to retry.
    #[doc(hidden)]
    pub fn close_once(&self) -> Result<(), CloseError> {
        self.handle.close()
    }

    /// Report whether the child can still accept runtime operations.
    #[doc(hidden)]
    pub fn is_active(&self) -> bool {
        self.handle.is_active()
    }
}

/// Borrowed owner capability. Its lifetime proves access to scope-local
/// typed payloads; it does not grant permission to close the owner.
#[derive(Clone, Copy)]
pub struct OwnerAccess<'owner> {
    pub(crate) storage: &'owner ScopeStorage,
    pub(crate) marker: PhantomData<&'owner ()>,
}

impl PartialEq for OwnerAccess<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.storage, other.storage)
    }
}

impl Eq for OwnerAccess<'_> {}

impl<'owner> OwnerAccess<'owner> {
    pub(crate) fn state(&self) -> runtime::ScopeState<'owner> {
        self.storage.owner_token().state()
    }

    pub fn with_transient<R>(
        &self,
        f: impl for<'child> FnOnce(OwnerAccess<'child>) -> R,
    ) -> TransientScopeResult<R> {
        if !self.storage.is_active() {
            return Err(TransientScopeError::Runtime(ReactiveError::NoSuchNode));
        }
        let state = self.storage.owner_token().state();
        let scheduler = state
            .try_borrow()
            .map_err(|_| TransientScopeError::Runtime(ReactiveError::BorrowConflict))?
            .scheduler
            .clone();
        let storage = ScopeStorage::new_with_owner(
            scheduler.clone(),
            Some(self.storage.owner_id),
            OwnerMode::Transient,
        );
        let access = OwnerAccess {
            storage: &storage,
            marker: PhantomData,
        };
        let frame = runtime::ObserverFrame::push_child(scheduler, storage.owner_id);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(access)));
        let close = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            close_owner_tree(&storage).map_or(Ok(()), Err)
        }));
        drop(frame);
        finish_transient(result, close)
    }

    pub fn create_child(&self) -> ReactiveResult<OwnerHandle> {
        OwnerHandle::new_child(self.storage)
    }

    /// Create a hidden persistent child adapter for framework-owned branches.
    #[doc(hidden)]
    pub fn create_persistent_child(&self) -> ReactiveResult<PersistentOwnerAccess<'owner>> {
        OwnerHandle::new_child(self.storage)
            .and_then(|handle| PersistentOwnerAccess::from_handle(self.storage, handle))
    }

    pub fn is_active(&self) -> bool {
        self.storage.is_active()
    }

    /// Compare runtime scheduler identity without conflating owner identity.
    pub fn same_runtime(&self, other: &Self) -> bool {
        let left = self.storage.state.borrow().scheduler.clone();
        let right = other.storage.state.borrow().scheduler.clone();
        Rc::ptr_eq(&left, &right)
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> runtime::RuntimeSnapshot {
        self.state().borrow().runtime_snapshot()
    }

    pub fn completion_once<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionOnce<T, E>>
    where
        E: 'owner,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'owner,
    {
        create_completion_once(self.storage, self.storage.owner_token().state(), callback)
    }

    pub fn completion_sender<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionSender<T, E>>
    where
        E: 'owner,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'owner,
    {
        create_completion_sender(self.storage, self.storage.owner_token().state(), callback)
    }
}

impl<'owner> OwnerAccess<'owner> {
    /// Register a callback owned by this owner and return its RAII token.
    pub fn error_handler<E, F>(&self, handler: F) -> ReactiveResult<ErrorHandlerToken<'owner, E>>
    where
        E: 'owner,
        F: Fn(E) + 'owner,
    {
        let state = self.state();
        let record = Rc::new(HandlerRecord::new(
            handler,
            WeakOwnerToken::from_erased(self.storage.state.clone()),
        ));
        let owner: Rc<dyn HandlerOwner + 'owner> = record.clone();
        let key = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| {
                state.register_error_handler(ErrorHandlerEntry {
                    owner,
                    identity: record.identity(),
                })
            })?;
        record.set_key(key);
        Ok(ErrorHandlerToken::from_record(self.storage, key, record))
    }

    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> R {
        let state = self.state();
        runtime::with_untracked(&state, f)
    }

    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> ReactiveResult<R> {
        let state = self.state();
        runtime::with_batch(&state, f)
    }

    /// Register cleanup on the current effect, or on this owner when no
    /// computation is active. Cleanup reads are untracked.
    pub fn on_cleanup<E, F, H>(&self, f: F, error_handler: H) -> ReactiveResult<()>
    where
        E: 'owner,
        F: FnOnce() -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        if !state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .is_active()
        {
            return Err(ReactiveError::NoSuchNode);
        }
        let handler = error_handler
            .handler_ref()
            .lease()
            .map_err(ReactiveError::Handler)?;
        let thunk = CleanupThunk::new(f, handler);
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        state.register_cleanup(thunk);
        Ok(())
    }

    pub fn callback<T, E, F>(&self, f: F) -> ReactiveResult<Callback<'owner, T, E>>
    where
        T: 'owner,
        E: 'owner,
        F: FnMut(T) -> Result<(), E> + 'owner,
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

    pub fn effect<E, F, H>(
        &self,
        f: F,
        error_handler: H,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        E: 'owner,
        F: FnMut() -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        runtime::create_effect(self.storage, &state, f, error_handler.handler_ref()).map(|raw| {
            EffectHandle {
                handle: Handle::new(self.storage, raw),
            }
        })
    }

    /// Register a framework-owned effect as a root of this reactive owner.
    ///
    /// Unlike [`Self::effect`], this does not become a child of the currently
    /// running computation. The callback still owns nodes and cleanups it
    /// creates while it executes. This entry point is hidden from normal API
    /// documentation because it is reserved for framework lifecycle code.
    #[doc(hidden)]
    pub fn effect_detached<E, F, H>(
        &self,
        f: F,
        error_handler: H,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        E: 'owner,
        F: FnMut() -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        runtime::create_effect_detached(self.storage, &state, f, error_handler.handler_ref()).map(
            |raw| EffectHandle {
                handle: Handle::new(self.storage, raw),
            },
        )
    }

    pub fn effect_with_previous<T, E, F, H>(
        &self,
        f: F,
        error_handler: H,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        T: 'owner,
        E: 'owner,
        F: FnMut(Option<&T>) -> Result<T, E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        runtime::create_previous(self.storage, &state, f, error_handler.handler_ref()).map(|raw| {
            EffectHandle {
                handle: Handle::new(self.storage, raw),
            }
        })
    }

    pub fn watch_getter<T, E, G, C, H>(
        &self,
        getter: G,
        callback: C,
        error_handler: H,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        T: PartialEq + 'owner,
        E: 'owner,
        G: FnMut() -> Result<T, E> + 'owner,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        self.watch_getter_with_options(getter, callback, error_handler, WatchOptions::default())
    }

    pub fn watch_getter_with_options<T, E, G, C, H>(
        &self,
        getter: G,
        callback: C,
        error_handler: H,
        options: WatchOptions,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        T: PartialEq + 'owner,
        E: 'owner,
        G: FnMut() -> Result<T, E> + 'owner,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        runtime::create_watch(
            self.storage,
            &state,
            getter,
            callback,
            error_handler.handler_ref(),
            options,
        )
        .map(|raw| EffectHandle {
            handle: Handle::new(self.storage, raw),
        })
    }

    pub fn watch<T, E, G, C, H>(
        &self,
        getter: G,
        callback: C,
        error_handler: H,
        options: WatchOptions,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        T: PartialEq + 'owner,
        E: 'owner,
        G: FnMut() -> Result<T, E> + 'owner,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        self.watch_getter_with_options(getter, callback, error_handler, options)
    }

    /// Create a computed value that notifies dependents only when its output
    /// changes according to `PartialEq`.
    pub fn computed<T, E, F, H>(
        &self,
        f: F,
        error_handler: H,
    ) -> ComputationInitResult<Computed<'owner, T, E>, E>
    where
        T: PartialEq + 'owner,
        E: 'owner,
        F: FnMut() -> Result<T, E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        let created =
            runtime::create_computed(self.storage, &state, f, error_handler.handler_ref())?;
        let handle = Handle::new(self.storage, created.raw);
        Ok(Computed {
            handle,
            value: created.value,
            errors: created.errors,
            marker: PhantomData,
        })
    }

    /// Create a computed value that notifies dependents after every successful
    /// evaluation, even when the output compares equal to the previous value.
    pub fn computed_always<T, E, F, H>(
        &self,
        f: F,
        error_handler: H,
    ) -> ComputationInitResult<Computed<'owner, T, E>, E>
    where
        T: 'owner,
        E: 'owner,
        F: FnMut() -> Result<T, E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        let created =
            runtime::create_computed_always(self.storage, &state, f, error_handler.handler_ref())?;
        let handle = Handle::new(self.storage, created.raw);
        Ok(Computed {
            handle,
            value: created.value,
            errors: created.errors,
            marker: PhantomData,
        })
    }

    pub fn node_ref<T: 'owner>(&self) -> ReactiveResult<NodeRef<'owner, T>> {
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

    pub fn signal<T: 'owner>(
        &self,
        value: T,
    ) -> ReactiveResult<(ReadSignal<'owner, T>, WriteSignal<'owner, T>)> {
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

    pub fn rw_signal<T: 'owner>(&self, value: T) -> ReactiveResult<RwSignal<'owner, T>> {
        let (read, write) = self.signal(value)?;
        Ok(RwSignal { read, write })
    }

    pub fn stored<T: 'owner>(&self, value: T) -> ReactiveResult<StoredValue<'owner, T>> {
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
}

pub(crate) fn new_root(runtime_slot: Rc<Cell<bool>>) -> OwnerHandle {
    let storage = Rc::new(ScopeStorage::new_with_owner(
        runtime::GlobalScheduler::new(),
        None,
        OwnerMode::Root,
    ));
    OwnerHandle::new(storage, Some(runtime_slot))
}

pub(crate) fn new_transient<R>(
    f: impl for<'owner> FnOnce(OwnerAccess<'owner>) -> R,
) -> TransientScopeResult<R> {
    let scheduler = runtime::GlobalScheduler::new();
    let storage = ScopeStorage::new_with_owner(scheduler.clone(), None, OwnerMode::Transient);
    let access = OwnerAccess {
        storage: &storage,
        marker: PhantomData,
    };
    let frame = runtime::ObserverFrame::push_untracked(scheduler);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(access)));
    let close = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        close_owner_tree(&storage).map_or(Ok(()), Err)
    }));
    drop(frame);
    finish_transient(result, close)
}

fn finish_transient<R>(
    result: Result<R, Box<dyn std::any::Any + Send>>,
    close: Result<Result<(), CloseError>, Box<dyn std::any::Any + Send>>,
) -> TransientScopeResult<R> {
    match (result, close) {
        (Ok(value), Ok(Ok(()))) => Ok(value),
        (Ok(_), Ok(Err(error))) => Err(TransientScopeError::Close(error)),
        (Err(panic), _) => std::panic::resume_unwind(panic),
        (Ok(_), Err(panic)) => std::panic::resume_unwind(panic),
    }
}

/// Close an owner and its descendants through the same child-first
/// transaction used by root, persistent, and transient owners.
fn close_owner_tree(storage: &ScopeStorage) -> Option<CloseError> {
    let owned = storage.children.borrow().clone();
    let mut transaction = CloseTransaction::new();
    for child in owned.into_iter().rev() {
        if let Some(error) = close_owner_tree(&child) {
            transaction.push_error(ClosePhase::Child, CloseSource::Child, error);
        }
    }
    if let Err(error) = storage.dispose_untracked() {
        transaction.push_error(ClosePhase::Runtime, CloseSource::Owner, error);
    }
    transaction.finish()
}
