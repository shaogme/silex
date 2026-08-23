//! Unified owner handles and lifetime-bearing owner access.
//!
//! `OwnerHandle` owns the runtime close operation. `OwnerAccess` is a borrowed
//! typed view and is the only new API entry point that creates scope-local
//! payloads. Runtime identity and generation are checked by the existing
//! storage/scheduler boundary; the Rust lifetime remains carried by this
//! borrowed view and is never reconstructed from an owner id alone.

mod node;
mod storage;

pub(crate) use storage::{CloseOutcome, ScopeStorage};

use crate::{
    ComputationInitResult, ErrorHandlerInput, ErrorHandlerToken, ReactiveError, ReactiveResult,
    completion::{
        CompletionOnce, CompletionSender, create_completion_once, create_completion_once_detached,
        create_completion_sender, create_completion_sender_detached,
    },
    error::{ErrorHandlerEntry, HandlerOwner, HandlerRecord},
    handle::Handle,
    root::{
        CloseError, ClosePhase, CloseSource, CloseTransaction, TransientScopeError,
        TransientScopeResult,
    },
    runtime::storage::{CallbackThunk, CleanupThunk},
    runtime::{self, OwnerMode},
    unsafe_boundary::WeakOwnerToken,
};
pub use node::{
    Callback, Computed, EffectHandle, EffectPhase, NodeRef, ReadGuard, ReadSignal, Signal,
    StoredValue, WatchOptions, WriteGuard, WriteSignal,
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
        if !storage.is_active()? {
            return Err(ReactiveError::NoSuchNode);
        }
        let state = storage.owner_token().state();
        let scheduler = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .scheduler
            .clone();
        let child =
            ScopeStorage::new_with_owner(scheduler, Some(storage.owner_id), OwnerMode::Persistent)?;
        let child = Rc::new(child);
        child.link_parent(&storage.children)?;
        storage.children.insert(child.clone())?;
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

    /// Close this owner. Only a retryable close failure leaves the owner open.
    pub fn close(&self) -> Result<(), CloseError> {
        if self.closed.get() {
            return Ok(());
        }
        let outcome = close_owner_tree(&self.storage);
        if outcome.released {
            self.closed.set(true);
            if let Some(runtime_slot) = &self.runtime_slot {
                runtime_slot.set(false);
            }
        }
        outcome.error.map_or(Ok(()), Err)
    }

    /// Report whether this owner can still accept runtime operations.
    ///
    /// # Errors
    ///
    /// Returns [`ReactiveError::BorrowConflict`] when the owner or scheduler
    /// state is already dynamically borrowed.
    pub fn is_active(&self) -> ReactiveResult<bool> {
        if self.closed.get() {
            return Ok(false);
        }
        self.storage.is_active()
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> ReactiveResult<runtime::RuntimeSnapshot> {
        self.access().runtime_snapshot()
    }
}

impl Drop for OwnerHandle {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            self.storage.report_close_error(error);
        }
    }
}

/// Owner-bound child capability with explicit close authority.
///
/// The capability is branded by the lifetime of the parent [`OwnerAccess`]. An
/// access borrowed from the child is tied to the parent owner lifetime, so the
/// child close authority can be moved into an owner-root cleanup without
/// weakening the scope boundary. Runtime operations still reject the access
/// after close. Closing the child is idempotent even when an ancestor has
/// already recursively closed it.
pub struct OwnerChild<'parent> {
    handle: OwnerHandle,
    marker: PhantomData<&'parent ()>,
}

/// Error returned when an owner-root cleanup registration cannot accept its
/// payload. The payload is returned unchanged so the caller can explicitly
/// roll it back.
pub struct OwnerCleanupRegistrationError<'owner, T> {
    error: ReactiveError,
    payload: T,
    marker: PhantomData<&'owner ()>,
}

impl<'owner, T> OwnerCleanupRegistrationError<'owner, T> {
    /// Recover both the registration error and the original payload.
    pub fn into_parts(self) -> (ReactiveError, T) {
        (self.error, self.payload)
    }
}

impl<'parent> OwnerChild<'parent> {
    fn from_handle(parent: &'parent ScopeStorage, handle: OwnerHandle) -> ReactiveResult<Self> {
        if !parent
            .children
            .contains(handle.storage.owner_id, &handle.storage)?
        {
            return Err(ReactiveError::InvariantViolation);
        }
        Ok(Self {
            handle,
            marker: PhantomData,
        })
    }

    /// Borrow the typed access for this owner-bound child.
    pub fn access(&self) -> OwnerAccess<'parent> {
        // `OwnerChild` owns the child storage and the parent brand keeps this
        // capability inside the parent's lifetime domain.
        let storage = self.handle.storage.as_ref() as *const ScopeStorage;
        let storage = unsafe { &*storage };
        OwnerAccess {
            storage,
            marker: PhantomData,
        }
    }

    /// Close the child from the caller's perspective.
    ///
    /// If a parent owner already closed the child, the runtime's released
    /// phase makes this a successful no-op. Only retryable close failures can
    /// be retried; a released child is never disposed twice.
    pub fn close(&self) -> Result<(), CloseError> {
        self.handle.close()
    }

    /// Report whether the child can still accept runtime operations.
    #[doc(hidden)]
    ///
    /// # Errors
    ///
    /// Returns [`ReactiveError::BorrowConflict`] when the child or scheduler
    /// state is already dynamically borrowed.
    pub fn is_active(&self) -> ReactiveResult<bool> {
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
        if !self
            .storage
            .is_active()
            .map_err(TransientScopeError::Runtime)?
        {
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
        )
        .map_err(TransientScopeError::Runtime)?;
        let access = OwnerAccess {
            storage: &storage,
            marker: PhantomData,
        };
        let frame = runtime::ObserverFrame::push_child(scheduler, storage.owner_id)
            .map_err(TransientScopeError::Runtime)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(access)));
        let close =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| close_owner_tree(&storage)));
        drop(frame);
        finish_transient(result, close)
    }

    pub fn create_child(&self) -> ReactiveResult<OwnerHandle> {
        OwnerHandle::new_child(self.storage)
    }

    /// Create an owner-bound child capability.
    pub fn create_owned_child(&self) -> ReactiveResult<OwnerChild<'owner>> {
        OwnerHandle::new_child(self.storage)
            .and_then(|handle| OwnerChild::from_handle(self.storage, handle))
    }

    /// Report whether this borrowed owner can still accept runtime operations.
    ///
    /// # Errors
    ///
    /// Returns [`ReactiveError::BorrowConflict`] when the owner or scheduler
    /// state is already dynamically borrowed.
    pub fn is_active(&self) -> ReactiveResult<bool> {
        self.storage.is_active()
    }

    /// Compare runtime scheduler identity without conflating owner identity.
    ///
    /// # Errors
    ///
    /// Returns [`ReactiveError::BorrowConflict`] when either owner or its
    /// scheduler is already dynamically borrowed.
    pub fn same_runtime(&self, other: &Self) -> ReactiveResult<bool> {
        let left = self
            .storage
            .owner_token()
            .state()
            .try_borrow()?
            .scheduler
            .clone();
        let right = other
            .storage
            .owner_token()
            .state()
            .try_borrow()?
            .scheduler
            .clone();
        let _left_scheduler = left
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let _right_scheduler = right
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        Ok(Rc::ptr_eq(&left, &right))
    }

    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn runtime_snapshot(&self) -> ReactiveResult<runtime::RuntimeSnapshot> {
        let mut snapshot = self.state().try_borrow()?.runtime_snapshot()?;
        snapshot.retained_children = self.storage.retained_children();
        let (typed_slots, error_slots) = self.storage.live_allocations();
        snapshot.live_typed_slots = typed_slots;
        snapshot.live_error_slots = error_slots;
        Ok(snapshot)
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

    #[doc(hidden)]
    pub fn completion_once_detached<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionOnce<T, E>>
    where
        E: 'owner,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'owner,
    {
        create_completion_once_detached(self.storage, self.storage.owner_token().state(), callback)
    }

    #[doc(hidden)]
    pub fn completion_sender_detached<T: 'static, E, F>(
        &self,
        callback: F,
    ) -> ReactiveResult<CompletionSender<T, E>>
    where
        E: 'owner,
        F: FnMut(T) -> Result<(), E> + UnwindSafe + 'owner,
    {
        create_completion_sender_detached(
            self.storage,
            self.storage.owner_token().state(),
            callback,
        )
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

    /// Run a callback without collecting reactive dependencies.
    ///
    /// # Errors
    ///
    /// Returns [`ReactiveError::BorrowConflict`] when the observer stack is
    /// already dynamically borrowed.
    ///
    /// # Panics
    ///
    /// Propagates a panic raised by `f` after restoring the observer stack.
    pub fn untrack<R>(&self, f: impl FnOnce() -> R) -> ReactiveResult<R> {
        let state = self.state();
        runtime::with_untracked(&state, f)
    }

    /// Run a callback without collecting dependencies while rejecting reads
    /// from a different runtime scheduler.
    #[doc(hidden)]
    pub fn with_runtime<R>(&self, f: impl FnOnce() -> R) -> ReactiveResult<R> {
        let state = self.state();
        runtime::with_runtime(&state, f)
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
            .is_active()?
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
        if !state.is_active()? {
            return Err(ReactiveError::NoSuchNode);
        }
        state.register_cleanup(runtime::CleanupTarget::CurrentOwner, thunk);
        Ok(())
    }

    /// Register a payload on this owner's root cleanup boundary.
    ///
    /// Unlike [`Self::on_cleanup`], this method never consults the currently
    /// running computation. The payload remains with the caller until all
    /// registration checks have succeeded; recoverable failures return it in
    /// [`OwnerCleanupRegistrationError`].
    pub fn on_owner_cleanup<T, E, F, H>(
        &self,
        payload: T,
        cleanup: F,
        error_handler: H,
    ) -> Result<(), OwnerCleanupRegistrationError<'owner, T>>
    where
        T: 'owner,
        E: 'owner,
        F: FnOnce(T) -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        let active = match state.try_borrow() {
            Ok(state) => match state.is_active() {
                Ok(active) => active,
                Err(error) => {
                    return Err(OwnerCleanupRegistrationError {
                        error,
                        payload,
                        marker: PhantomData,
                    });
                }
            },
            Err(error) => {
                return Err(OwnerCleanupRegistrationError {
                    error,
                    payload,
                    marker: PhantomData,
                });
            }
        };
        if !active {
            return Err(OwnerCleanupRegistrationError {
                error: ReactiveError::NoSuchNode,
                payload,
                marker: PhantomData,
            });
        }
        let handler = match error_handler.handler_ref().lease() {
            Ok(handler) => handler,
            Err(error) => {
                return Err(OwnerCleanupRegistrationError {
                    error: ReactiveError::Handler(error),
                    payload,
                    marker: PhantomData,
                });
            }
        };
        let mut state = match state.try_borrow_mut() {
            Ok(state) => state,
            Err(error) => {
                return Err(OwnerCleanupRegistrationError {
                    error,
                    payload,
                    marker: PhantomData,
                });
            }
        };
        if !match state.is_active() {
            Ok(active) => active,
            Err(error) => {
                return Err(OwnerCleanupRegistrationError {
                    error,
                    payload,
                    marker: PhantomData,
                });
            }
        } {
            return Err(OwnerCleanupRegistrationError {
                error: ReactiveError::NoSuchNode,
                payload,
                marker: PhantomData,
            });
        }
        let thunk = CleanupThunk::new(move || cleanup(payload), handler);
        state.register_cleanup(runtime::CleanupTarget::OwnerRoot, thunk);
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
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_callback(thunk))?;
        let callback = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .typed_node_ref(raw)?;
        Ok(Callback {
            handle: Handle::new(self.storage, raw),
            callback,
            marker: PhantomData,
        })
    }

    pub fn effect<E, F, H>(
        &self,
        phase: EffectPhase,
        f: F,
        error_handler: H,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        E: 'owner,
        F: FnMut() -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        runtime::create_effect(self.storage, &state, phase, f, error_handler.handler_ref()).map(
            |raw| EffectHandle {
                handle: Handle::new(self.storage, raw),
            },
        )
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
        phase: EffectPhase,
        f: F,
        error_handler: H,
    ) -> ComputationInitResult<EffectHandle<'owner>, E>
    where
        E: 'owner,
        F: FnMut() -> Result<(), E> + 'owner,
        H: ErrorHandlerInput<'owner, E>,
    {
        let state = self.state();
        runtime::create_effect_detached(self.storage, &state, phase, f, error_handler.handler_ref())
            .map(|raw| EffectHandle {
                handle: Handle::new(self.storage, raw),
            })
    }

    pub fn effect_with_previous<T, E, F, H>(
        &self,
        phase: EffectPhase,
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
        runtime::create_previous(self.storage, &state, phase, f, error_handler.handler_ref()).map(
            |raw| EffectHandle {
                handle: Handle::new(self.storage, raw),
            },
        )
    }

    pub fn watch_getter<T, E, G, C, H>(
        &self,
        phase: EffectPhase,
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
        self.watch_getter_with_options(
            phase,
            getter,
            callback,
            error_handler,
            WatchOptions::default(),
        )
    }

    pub fn watch_getter_with_options<T, E, G, C, H>(
        &self,
        phase: EffectPhase,
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
            phase,
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
        phase: EffectPhase,
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
        self.watch_getter_with_options(phase, getter, callback, error_handler, options)
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
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_node_ref(slot))?;
        let value = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .typed_node_ref(raw)?;
        Ok(NodeRef {
            handle: Handle::new(self.storage, raw),
            value,
            marker: PhantomData,
        })
    }

    pub fn signal<T: 'owner>(&self, value: T) -> ReactiveResult<Signal<'owner, T>> {
        let slot = self.storage.alloc_slot(value);
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_signal(slot))?;
        let value_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .typed_node_ref(raw)?;
        let handle = Handle::new(self.storage, raw);
        Signal::from_pair((
            ReadSignal {
                handle,
                value: value_ref,
                marker: PhantomData,
            },
            WriteSignal {
                handle,
                value: value_ref,
                marker: PhantomData,
            },
        ))
    }

    pub fn stored<T: 'owner>(&self, value: T) -> ReactiveResult<StoredValue<'owner, T>> {
        let slot = self.storage.alloc_slot(value);
        let state = self.state();
        let raw = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
            .and_then(|mut state| state.create_stored(slot))?;
        let value_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .typed_node_ref(raw)?;
        Ok(StoredValue {
            handle: Handle::new(self.storage, raw),
            value: value_ref,
            marker: PhantomData,
        })
    }
}

pub(crate) fn new_root(
    runtime_slot: Rc<Cell<bool>>,
    close_reports: Rc<runtime::CloseReportQueue>,
) -> ReactiveResult<OwnerHandle> {
    let storage = Rc::new(ScopeStorage::new_with_owner(
        runtime::GlobalScheduler::new_with_reporter(close_reports),
        None,
        OwnerMode::Root,
    )?);
    Ok(OwnerHandle::new(storage, Some(runtime_slot)))
}

pub(crate) fn new_transient<R>(
    f: impl for<'owner> FnOnce(OwnerAccess<'owner>) -> R,
    close_reports: Rc<runtime::CloseReportQueue>,
) -> TransientScopeResult<R> {
    let scheduler = runtime::GlobalScheduler::new_with_reporter(close_reports);
    let storage = ScopeStorage::new_with_owner(scheduler.clone(), None, OwnerMode::Transient)
        .map_err(TransientScopeError::Runtime)?;
    let access = OwnerAccess {
        storage: &storage,
        marker: PhantomData,
    };
    let frame =
        runtime::ObserverFrame::push_untracked(scheduler).map_err(TransientScopeError::Runtime)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(access)));
    let close =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| close_owner_tree(&storage)));
    drop(frame);
    finish_transient(result, close)
}

fn finish_transient<R>(
    result: Result<R, Box<dyn std::any::Any + Send>>,
    close: Result<CloseOutcome, Box<dyn std::any::Any + Send>>,
) -> TransientScopeResult<R> {
    match (result, close) {
        (Ok(value), Ok(outcome)) if outcome.released && outcome.error.is_none() => Ok(value),
        (Ok(_), Ok(outcome)) => Err(TransientScopeError::Close(outcome.error.unwrap_or_else(
            || CloseError::from_panic(Box::new("transient close did not produce a diagnostic")),
        ))),
        (Err(panic), _) => std::panic::resume_unwind(panic),
        (Ok(_), Err(panic)) => std::panic::resume_unwind(panic),
    }
}

/// Close an owner and its descendants through the same child-first
/// transaction used by root, persistent, and transient owners.
fn close_owner_tree(storage: &ScopeStorage) -> CloseOutcome {
    let owned = match storage.children.snapshot() {
        Ok(owned) => owned,
        Err(error) => {
            return CloseOutcome::retryable(
                CloseError::from_failures(vec![crate::root::CleanupFailure::Runtime(error)])
                    .unwrap_or_else(|| {
                        CloseError::from_panic(Box::new(
                            "owner child registry did not produce a diagnostic",
                        ))
                    }),
            );
        }
    };
    let mut transaction = CloseTransaction::new();
    let mut retryable_child = false;
    for child in owned.into_iter().rev() {
        let outcome = close_owner_tree(&child);
        if !outcome.released {
            retryable_child = true;
        }
        if let Some(error) = outcome.error {
            transaction.push_error(ClosePhase::Child, CloseSource::Child, error);
        }
    }
    if retryable_child {
        return CloseOutcome {
            released: false,
            error: Some(transaction.finish().unwrap_or_else(|| {
                CloseError::from_panic(Box::new(
                    "owner close retained a retryable child without a diagnostic",
                ))
            })),
        };
    }
    let own = storage.dispose_untracked();
    if let Some(error) = own.error {
        transaction.push_error(ClosePhase::Runtime, CloseSource::Owner, error);
    }
    CloseOutcome {
        released: own.released,
        error: transaction.finish(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]

    use super::*;

    #[test]
    fn retryable_child_keeps_parent_until_a_later_close() {
        let scheduler = runtime::GlobalScheduler::new();
        let Some(parent_storage) =
            ScopeStorage::new_with_owner(scheduler.clone(), None, OwnerMode::Root).ok()
        else {
            return;
        };
        let parent = Rc::new(parent_storage);
        let Some(child_storage) =
            ScopeStorage::new_with_owner(scheduler, Some(parent.owner_id), OwnerMode::Transient)
                .ok()
        else {
            return;
        };
        let child = Rc::new(child_storage);
        assert!(child.link_parent(&parent.children).is_ok());
        assert!(parent.children.insert(child.clone()).is_ok());

        let child_state = child
            .state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict);
        let Ok(child_state) = child_state else {
            return;
        };
        let first = close_owner_tree(&parent);
        assert!(!first.released);
        assert!(parent.is_active().expect("parent active state"));
        drop(child_state);

        let second = close_owner_tree(&parent);
        assert!(second.released);
        assert!(!parent.is_active().expect("parent active state"));
    }

    #[test]
    fn adopted_child_is_closed_by_parent_root_cleanup() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let child = owner
            .create_owned_child()
            .expect("persistent child should initialize");
        let child_owner = child.access();
        let handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");

        assert!(
            owner
                .on_owner_cleanup(child, |child| child.close().map_err(|_| ()), handler.view(),)
                .is_ok()
        );
        assert!(child_owner.is_active().expect("child should be active"));

        root.close().expect("parent close should succeed");
        assert!(!child_owner.is_active().expect("child should be inactive"));
        assert!(
            runtime
                .take_unhandled_close_errors()
                .expect("close diagnostics")
                .is_empty()
        );
    }

    #[test]
    fn generic_payload_can_be_registered_at_owner_root() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let cleaned = Rc::new(Cell::new(false));
        let cleaned_for_cleanup = cleaned.clone();
        let payload = String::from("generic owner payload");
        let handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");

        owner
            .on_cleanup(
                move || {
                    assert_eq!(payload, "generic owner payload");
                    cleaned_for_cleanup.set(true);
                    Ok::<(), ()>(())
                },
                handler.view(),
            )
            .expect("generic payload cleanup should register");

        root.close().expect("parent close should succeed");
        assert!(cleaned.get());
    }

    #[test]
    fn owner_cleanup_accepts_generic_payload_inside_effect() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let source = owner.signal(false).expect("source signal");
        let registered = Rc::new(Cell::new(false));
        let cleaned = Rc::new(Cell::new(0));
        let effect_handler = owner
            .error_handler(|_: ReactiveError| {})
            .expect("effect handler should initialize");
        let cleanup_handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");
        let registered_for_effect = registered.clone();
        let cleaned_for_effect = cleaned.clone();

        owner
            .effect(
                EffectPhase::Normal,
                move || {
                    let _ = source.get()?;
                    if !registered_for_effect.replace(true) {
                        let cleaned_for_cleanup = cleaned_for_effect.clone();
                        owner
                            .on_owner_cleanup(
                                String::from("generic payload"),
                                move |payload| {
                                    assert_eq!(payload, "generic payload");
                                    cleaned_for_cleanup.set(cleaned_for_cleanup.get() + 1);
                                    Ok::<(), ()>(())
                                },
                                cleanup_handler.view(),
                            )
                            .map_err(|error| {
                                let (error, payload) = error.into_parts();
                                drop(payload);
                                error
                            })?;
                    }
                    Ok(())
                },
                effect_handler.view(),
            )
            .expect("effect should initialize");

        source.set(true).expect("source should rerun effect");
        assert_eq!(cleaned.get(), 0);
        root.close().expect("parent close should succeed");
        assert_eq!(cleaned.get(), 1);
    }

    #[test]
    fn owner_cleanup_stays_at_owner_root_during_effect_rerun() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let source = owner.signal(false).expect("source signal");
        let child_access = Rc::new(Cell::new(None));
        let child_access_for_effect = child_access.clone();
        let effect_handler = owner
            .error_handler(|_: ReactiveError| {})
            .expect("effect handler should initialize");
        let cleanup_handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");

        owner
            .effect(
                EffectPhase::Normal,
                move || {
                    let _ = source.get()?;
                    if child_access_for_effect.get().is_none() {
                        let child = owner.create_owned_child()?;
                        child_access_for_effect.set(Some(child.access()));
                        owner
                            .on_owner_cleanup(
                                child,
                                |child| child.close().map_err(|_| ()),
                                cleanup_handler.view(),
                            )
                            .map_err(|error| {
                                let (error, child) = error.into_parts();
                                let _ = child.close();
                                error
                            })?;
                    }
                    Ok(())
                },
                effect_handler.view(),
            )
            .expect("effect should initialize");

        assert!(
            child_access
                .get()
                .expect("child access should be stored")
                .is_active()
                .expect("child should be active")
        );
        source.set(true).expect("source should rerun effect");
        assert!(
            child_access
                .get()
                .expect("child access should remain stored")
                .is_active()
                .expect("child should remain active after effect rerun")
        );

        root.close().expect("parent close should succeed");
        assert!(
            !child_access
                .get()
                .expect("child access should remain stored")
                .is_active()
                .expect("child should be inactive after parent close")
        );
    }

    #[test]
    fn owner_cleanup_returns_payload_when_handler_lease_fails() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");
        let stale_handler = handler.view();
        drop(handler);

        let result = owner.on_owner_cleanup(
            String::from("rollback payload"),
            |_| Ok::<(), ()>(()),
            stale_handler,
        );
        let error = result.expect_err("stale handler must reject registration");
        let (error, payload) = error.into_parts();
        assert!(matches!(error, ReactiveError::Handler(_)));
        assert_eq!(payload, "rollback payload");
        root.close().expect("parent close should succeed");
    }

    #[test]
    fn owner_cleanup_returns_payload_when_borrow_conflicts() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");
        let state = owner.state();
        let borrow = state
            .try_borrow_mut()
            .expect("test should hold the state borrow");

        let result = owner.on_owner_cleanup(
            String::from("borrow rollback payload"),
            |_| Ok::<(), ()>(()),
            handler.view(),
        );
        let error = result.expect_err("state borrow must reject registration");
        let (error, payload) = error.into_parts();
        assert_eq!(error, ReactiveError::BorrowConflict);
        assert_eq!(payload, "borrow rollback payload");
        drop(borrow);
        root.close().expect("parent close should succeed");
    }

    #[test]
    fn owner_cleanup_returns_payload_when_owner_is_inactive() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");
        root.close().expect("parent close should succeed");

        let result = owner.on_owner_cleanup(
            String::from("inactive rollback payload"),
            |_| Ok::<(), ()>(()),
            handler.view(),
        );
        let error = result.expect_err("inactive owner must reject registration");
        let (error, payload) = error.into_parts();
        assert_eq!(error, ReactiveError::NoSuchNode);
        assert_eq!(payload, "inactive rollback payload");
    }

    #[test]
    fn owned_child_supports_explicit_idempotent_close() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let child = root
            .access()
            .create_owned_child()
            .expect("owned child should initialize");
        let child_access = child.access();

        child.close().expect("child close should succeed");
        child
            .close()
            .expect("repeated child close should be a no-op");
        assert!(!child_access.is_active().expect("child active state"));
        root.close().expect("parent close should succeed");
    }

    #[test]
    fn adopted_child_cleanup_stays_at_owner_root_during_effect_rerun() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let source = owner.signal(false).expect("source signal");
        let adopted_owner = Rc::new(Cell::new(None));
        let adopted_owner_for_effect = adopted_owner.clone();
        let effect_handler = owner
            .error_handler(|_: ReactiveError| {})
            .expect("effect handler should initialize");
        let cleanup_handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");

        owner
            .effect(
                EffectPhase::Normal,
                move || {
                    let _ = source.get()?;
                    if adopted_owner_for_effect.get().is_none() {
                        let child = owner.create_owned_child()?;
                        adopted_owner_for_effect.set(Some(child.access()));
                        owner
                            .on_owner_cleanup(
                                child,
                                |child| child.close().map_err(|_| ()),
                                cleanup_handler.view(),
                            )
                            .map_err(|error| {
                                let (error, child) = error.into_parts();
                                let _ = child.close();
                                error
                            })?;
                    }
                    Ok(())
                },
                effect_handler.view(),
            )
            .expect("effect should initialize");
        assert!(
            adopted_owner
                .get()
                .expect("child access should be stored")
                .is_active()
                .expect("child should be active")
        );

        source.set(true).expect("source should rerun effect");
        assert!(
            adopted_owner
                .get()
                .expect("child access should remain stored")
                .is_active()
                .expect("effect rerun must not close child")
        );

        root.close().expect("parent close should succeed");
        assert!(
            !adopted_owner
                .get()
                .expect("child access should remain stored")
                .is_active()
                .expect("parent close should close child")
        );
    }

    #[test]
    fn failed_child_adoption_returns_authority_for_rollback() {
        let mut runtime = crate::runtime::Runtime::new();
        let root = runtime.owner().expect("root owner");
        let owner = root.access();
        let child = owner
            .create_owned_child()
            .expect("persistent child should initialize");
        let child_owner = child.access();
        let handler = owner
            .error_handler(|_: ()| {})
            .expect("cleanup handler should initialize");
        let stale_handler = handler.view();
        drop(handler);

        let result =
            owner.on_owner_cleanup(child, |child| child.close().map_err(|_| ()), stale_handler);
        let error = match result {
            Ok(()) => panic!("stale handler must reject adoption"),
            Err(error) => error,
        };
        let (error, child) = error.into_parts();
        assert!(matches!(error, ReactiveError::Handler(_)));
        assert!(child_owner.is_active().expect("child should remain active"));
        child.close().expect("rollback should close the child");
        assert!(!child_owner.is_active().expect("child should be inactive"));

        root.close().expect("parent close should succeed");
        assert!(
            runtime
                .take_unhandled_close_errors()
                .expect("close diagnostics")
                .is_empty()
        );
    }
}
