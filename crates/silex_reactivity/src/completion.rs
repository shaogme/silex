//! Scope-owned completion destinations for asynchronous tasks.

use crate::{
    error::{CallbackInvokeError, CompletionSubmitResult, ReactiveError, map_callback_error},
    internal::NodeId,
    owner::ScopeStorage,
    root::{CleanupFailure, CloseError},
    runtime::storage::{CallbackThunk, CallbackThunkError, TypedNodeRef, TypedSlot},
    runtime::{self, GlobalScheduler, OwnerId, ScopeState},
    unsafe_boundary::{OwnerToken, WeakOwnerToken},
};
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, UnwindSafe, catch_unwind, resume_unwind},
    ptr::NonNull,
    rc::Rc,
};

struct ActiveOwner<'scope> {
    owner: OwnerToken<'scope>,
    state: ScopeState<'scope>,
}

/// Wrap a repeating callback with an explicit unwind-safety assertion.
///
/// `AssertUnwindSafe` itself only implements `FnOnce`; this adapter preserves
/// the `FnMut` contract required by repeating completion destinations.
pub fn unwind_safe<T, E, F>(callback: F) -> impl FnMut(T) -> Result<(), E> + UnwindSafe
where
    F: FnMut(T) -> Result<(), E>,
{
    let mut callback = AssertUnwindSafe(callback);
    move |value| (*callback)(value)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionPhase {
    Active,
    Completing,
    Closing,
    Closed,
}

/// The only typed lifetime adapter retained by the completion endpoint.
///
/// A normal callback handle keeps a typed reference directly. A completion
/// sender must be `'static` with respect to the submitted value while its
/// callback may remain owner-local, so the endpoint stores only a pointer and
/// restores it after the runtime validator has checked owner/node identity and
/// phase. No other module can construct or restore this representation.
#[derive(Clone, Copy)]
struct TypedCompletionEndpoint<T, E> {
    pointer: NonNull<()>,
    marker: PhantomData<fn(T) -> E>,
}

impl<T, E> TypedCompletionEndpoint<T, E> {
    /// # Safety
    ///
    /// `callback` must point into the owner arena and the callback node must
    /// be registered before the endpoint is published. The owner close path
    /// clears the typed slot before releasing that arena.
    unsafe fn from_callback<'scope>(
        callback: TypedNodeRef<'scope, CallbackThunk<'scope, T, E>>,
    ) -> Self {
        Self {
            pointer: NonNull::from(callback.slot()).cast(),
            marker: PhantomData,
        }
    }

    /// # Safety
    ///
    /// The caller must have just completed the runtime owner/node/phase
    /// validation for the endpoint that owns this pointer.
    unsafe fn restore<'scope>(
        &self,
        _owner: &OwnerToken<'scope>,
    ) -> TypedNodeRef<'scope, CallbackThunk<'scope, T, E>> {
        let slot = unsafe {
            self.pointer
                .cast::<TypedSlot<CallbackThunk<'scope, T, E>>>()
                .as_ref()
        };
        TypedNodeRef::from_slot(slot)
    }
}

struct CompletionEndpoint<T, E> {
    state: WeakOwnerToken,
    scheduler: Rc<RefCell<GlobalScheduler>>,
    owner_id: OwnerId,
    callback: NodeId,
    typed_callback: TypedCompletionEndpoint<T, E>,
    phase: Cell<CompletionPhase>,
}

impl<T, E> CompletionEndpoint<T, E> {
    fn new(
        state: WeakOwnerToken,
        scheduler: Rc<RefCell<GlobalScheduler>>,
        owner_id: OwnerId,
        callback: NodeId,
        typed_callback: TypedCompletionEndpoint<T, E>,
    ) -> Self {
        Self {
            state,
            scheduler,
            owner_id,
            callback,
            typed_callback,
            phase: Cell::new(CompletionPhase::Active),
        }
    }

    fn current_owner<'scope>(&self) -> Result<Option<ActiveOwner<'scope>>, ReactiveError> {
        let owner = self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .resolve_owner(self.owner_id, &self.state);
        let Some(owner) = owner else {
            return Ok(None);
        };
        let state = owner.state();
        Ok(Some(ActiveOwner { owner, state }))
    }

    fn current_state<'scope>(&self) -> Result<Option<ScopeState<'scope>>, ReactiveError> {
        Ok(self.current_owner()?.map(|active| active.state))
    }

    fn validated_callback<'scope>(&self) -> Result<Option<ActiveOwner<'scope>>, ReactiveError> {
        let Some(active) = self.current_owner()? else {
            return Ok(None);
        };
        active.state.validate_callback_endpoint(self.callback)?;
        Ok(Some(active))
    }

    fn begin_once<'scope>(&self) -> Result<Option<ActiveOwner<'scope>>, ReactiveError> {
        if self.phase.replace(CompletionPhase::Completing) != CompletionPhase::Active {
            return Ok(None);
        }
        match self.validated_callback()? {
            Some(owner) => Ok(Some(owner)),
            None => {
                self.phase.set(CompletionPhase::Closed);
                Ok(None)
            }
        }
    }

    fn close_and_dispose(&self) -> Result<(), CloseError> {
        if self.phase.get() == CompletionPhase::Closed {
            return Ok(());
        }
        self.phase.set(CompletionPhase::Closing);
        let Some(state) = self.current_state().map_err(close_runtime_error)? else {
            self.phase.set(CompletionPhase::Closed);
            return Ok(());
        };
        let outcome =
            runtime::dispose_nodes(&state, vec![self.callback]).map_err(close_runtime_error)?;
        let mut failures = outcome
            .runtime_errors
            .into_iter()
            .map(CleanupFailure::Runtime)
            .collect::<Vec<_>>();
        failures.extend(
            outcome
                .handler_errors
                .into_iter()
                .map(CleanupFailure::Handler),
        );
        failures.extend(outcome.panics.into_iter().map(CloseError::panic_failure));
        if let Some(error) = CloseError::from_failures(failures) {
            return Err(error);
        }
        self.phase.set(CompletionPhase::Closed);
        Ok(())
    }

    fn submit_repeating(&self, value: T) -> Result<bool, CallbackThunkError<E>> {
        if self.phase.get() != CompletionPhase::Active {
            return Ok(false);
        }
        let active = match self.validated_callback() {
            Ok(Some(active)) => active,
            Ok(None) => return Ok(false),
            Err(error) => return Err(CallbackThunkError::Runtime(error)),
        };
        // SAFETY: `validated_callback` checked the scheduler identity, owner
        // generation, active phase, callback node generation, and callback
        // node kind immediately before this restore.
        let typed_callback = unsafe { self.typed_callback.restore(&active.owner) };
        runtime::invoke_callback(&active.state, self.callback, typed_callback, value).map(|()| true)
    }
}

fn close_runtime_error(error: ReactiveError) -> CloseError {
    CloseError::from_failures(vec![CleanupFailure::Runtime(error)])
        .expect("a runtime close failure must produce a close error")
}

fn drop_completion_state<T, E>(state: &CompletionEndpoint<T, E>) {
    let _ = catch_unwind(AssertUnwindSafe(|| state.close_and_dispose()));
}

/// A destination that accepts one completion and then disposes its callback node.
///
/// Clones share the same terminal state. Dropping the final active clone cancels
/// the destination without invoking the user callback.
pub struct CompletionOnce<T, E> {
    state: Rc<CompletionEndpoint<T, E>>,
    marker: PhantomData<fn(T) -> E>,
}

impl<T, E> Clone for CompletionOnce<T, E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}

impl<T, E> Drop for CompletionOnce<T, E> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.state) == 1 {
            drop_completion_state(&self.state);
        }
    }
}

impl<T: 'static, E> CompletionOnce<T, E> {
    pub fn submit(&self, value: T) -> CompletionSubmitResult<E> {
        let ActiveOwner { owner, state } = match self.state.begin_once() {
            Ok(Some(owner)) => owner,
            Ok(None) => return Ok(false),
            Err(error) => {
                let _ = self.state.close_and_dispose();
                return Err(CallbackInvokeError::Runtime(error));
            }
        };
        let callback_result = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: `begin_once` validates the owner and this endpoint's
            // node is validated before restoring its typed callback.
            let typed_callback = unsafe { self.state.typed_callback.restore(&owner) };
            runtime::invoke_callback(&state, self.state.callback, typed_callback, value)
        }));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| self.state.close_and_dispose()));

        match (callback_result, dispose_result) {
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
            (Ok(Ok(())), Ok(Ok(()))) => Ok(true),
            (Ok(Ok(())), Ok(Err(error))) => Err(CallbackInvokeError::Close(error)),
            (Ok(Err(error)), Ok(Ok(()))) => Err(map_callback_error(error)),
            (Ok(Err(error)), Ok(Err(_))) => Err(map_callback_error(error)),
        }
    }

    pub fn cancel(&self) -> Result<(), CloseError> {
        self.state.close_and_dispose()
    }
}

/// A cloneable destination for a callback that may receive multiple messages.
///
/// The final active clone cancels the callback node. Explicit cancellation is
/// still required when a long-lived owner is replaced before all senders drop.
/// A callback panic is terminal: the callback node is disposed before the panic
/// is resumed, and later submissions return `Ok(false)`.
pub struct CompletionSender<T, E> {
    state: Rc<CompletionEndpoint<T, E>>,
    marker: PhantomData<fn(T) -> E>,
}

impl<T, E> Clone for CompletionSender<T, E> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}

impl<T, E> Drop for CompletionSender<T, E> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.state) == 1 {
            drop_completion_state(&self.state);
        }
    }
}

impl<T: 'static, E> CompletionSender<T, E> {
    pub fn submit(&self, value: T) -> CompletionSubmitResult<E> {
        let callback_result = catch_unwind(AssertUnwindSafe(|| self.state.submit_repeating(value)));
        match callback_result {
            Ok(result) => result.map_err(map_callback_error),
            Err(callback_panic) => {
                let _ = catch_unwind(AssertUnwindSafe(|| self.state.close_and_dispose()));
                resume_unwind(callback_panic)
            }
        }
    }

    pub fn cancel(&self) -> Result<(), CloseError> {
        self.state.close_and_dispose()
    }
}

fn create_completion_state<'scope, T: 'static, E, F>(
    storage: &'scope ScopeStorage,
    state: ScopeState<'scope>,
    callback: F,
) -> Result<Rc<CompletionEndpoint<T, E>>, ReactiveError>
where
    E: 'scope,
    F: FnMut(T) -> Result<(), E> + 'scope,
{
    let scheduler = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state_ref.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        state_ref.scheduler.clone()
    };

    let thunk = storage.alloc_slot(CallbackThunk::new(callback));
    let callback = match state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)
        .and_then(|mut state_ref| state_ref.create_callback(thunk))
    {
        Ok(callback) => callback,
        Err(error) => {
            thunk.slot().clear();
            return Err(error);
        }
    };
    let weak = WeakOwnerToken::from_typed(&state);
    // SAFETY: The callback slot is registered before this endpoint is
    // published and is cleared by the unified node disposal path.
    let typed_callback = unsafe { TypedCompletionEndpoint::from_callback(thunk) };
    Ok(Rc::new(CompletionEndpoint::new(
        weak,
        scheduler,
        storage.owner_id,
        callback,
        typed_callback,
    )))
}

pub(crate) fn create_completion_once<'scope, T: 'static, E, F>(
    storage: &'scope ScopeStorage,
    state: ScopeState<'scope>,
    callback: F,
) -> Result<CompletionOnce<T, E>, ReactiveError>
where
    E: 'scope,
    F: FnMut(T) -> Result<(), E> + 'scope,
{
    Ok(CompletionOnce {
        state: create_completion_state(storage, state, callback)?,
        marker: PhantomData,
    })
}

pub(crate) fn create_completion_sender<'scope, T: 'static, E, F>(
    storage: &'scope ScopeStorage,
    state: ScopeState<'scope>,
    callback: F,
) -> Result<CompletionSender<T, E>, ReactiveError>
where
    E: 'scope,
    F: FnMut(T) -> Result<(), E> + 'scope,
{
    Ok(CompletionSender {
        state: create_completion_state(storage, state, callback)?,
        marker: PhantomData,
    })
}
