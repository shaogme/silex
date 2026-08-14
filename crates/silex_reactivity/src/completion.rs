//! Scope-owned completion destinations for asynchronous tasks.

use crate::{
    error::{CallbackInvokeError, CompletionSubmitResult, ReactiveError, map_callback_error},
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk, CallbackThunkError},
    },
    runtime::{self, GlobalScheduler, ScopeId, ScopeState},
    scope::ScopeStorage,
    unsafe_boundary::WeakOwnerToken,
};
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, UnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

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
    Closed,
}

struct CompletionState {
    state: WeakOwnerToken,
    scheduler: Rc<RefCell<GlobalScheduler>>,
    scope_id: ScopeId,
    callback: RawId,
    phase: Cell<CompletionPhase>,
}

impl CompletionState {
    fn new(
        state: WeakOwnerToken,
        scheduler: Rc<RefCell<GlobalScheduler>>,
        scope_id: ScopeId,
        callback: RawId,
    ) -> Self {
        Self {
            state,
            scheduler,
            scope_id,
            callback,
            phase: Cell::new(CompletionPhase::Active),
        }
    }

    fn current_state<'scope>(
        &self,
    ) -> Result<Option<Rc<RefCell<ScopeState<'scope>>>>, ReactiveError> {
        if !self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .is_scope_current(self.scope_id, &self.state)
        {
            return Ok(None);
        }

        let Some(owner) = self.state.upgrade() else {
            return Ok(None);
        };
        Ok(Some(owner.state()))
    }

    fn begin_once<'scope>(&self) -> Result<Option<Rc<RefCell<ScopeState<'scope>>>>, ReactiveError> {
        if self.phase.replace(CompletionPhase::Completing) != CompletionPhase::Active {
            return Ok(None);
        }
        match self.current_state()? {
            Some(state) => Ok(Some(state)),
            None => {
                self.phase.set(CompletionPhase::Closed);
                Ok(None)
            }
        }
    }

    fn close_and_dispose(&self) {
        if self.phase.replace(CompletionPhase::Closed) == CompletionPhase::Closed {
            return;
        }
        if let Ok(Some(state)) = self.current_state() {
            runtime::dispose_nodes(&state, vec![self.callback]);
        }
    }

    fn submit_repeating<'scope, T: 'static>(
        &self,
        value: T,
    ) -> Result<bool, CallbackThunkError<'scope>> {
        if self.phase.get() != CompletionPhase::Active {
            return Ok(false);
        }
        let state = match self.current_state() {
            Ok(Some(state)) => state,
            Ok(None) => return Ok(false),
            Err(error) => return Err(CallbackThunkError::Runtime(error)),
        };
        runtime::invoke_callback(&state, self.callback, AnyValue::new(value)).map(|()| true)
    }
}

fn drop_completion_state(state: &CompletionState) {
    let result = catch_unwind(AssertUnwindSafe(|| state.close_and_dispose()));
    if let Err(panic) = result
        && !std::thread::panicking()
    {
        resume_unwind(panic);
    }
}

/// A destination that accepts one completion and then disposes its callback node.
///
/// Clones share the same terminal state. Dropping the final active clone cancels
/// the destination without invoking the user callback.
pub struct CompletionOnce<T, E> {
    state: Rc<CompletionState>,
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
    pub fn submit<'scope>(&self, value: T) -> CompletionSubmitResult<E>
    where
        E: 'scope,
    {
        let state = match self.state.begin_once() {
            Ok(Some(state)) => state,
            Ok(None) => return Ok(false),
            Err(error) => {
                self.state.close_and_dispose();
                return Err(CallbackInvokeError::Runtime(error));
            }
        };
        let callback_result = catch_unwind(AssertUnwindSafe(|| {
            runtime::invoke_callback(&state, self.state.callback, AnyValue::new(value))
        }));
        let dispose_result = catch_unwind(AssertUnwindSafe(|| {
            self.state.close_and_dispose();
        }));

        match (callback_result, dispose_result) {
            (Err(panic), _) => resume_unwind(panic),
            (Ok(_), Err(panic)) => resume_unwind(panic),
            (Ok(Ok(())), Ok(())) => Ok(true),
            (Ok(Err(error)), Ok(())) => Err(map_callback_error(error)),
        }
    }

    pub fn cancel(&self) {
        self.state.close_and_dispose();
    }
}

/// A cloneable destination for a callback that may receive multiple messages.
///
/// The final active clone cancels the callback node. Explicit cancellation is
/// still required when a long-lived owner is replaced before all senders drop.
/// A callback panic is terminal: the callback node is disposed before the panic
/// is resumed, and later submissions return `Ok(false)`.
pub struct CompletionSender<T, E> {
    state: Rc<CompletionState>,
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
    pub fn submit<'scope>(&self, value: T) -> CompletionSubmitResult<E>
    where
        E: 'scope,
    {
        let callback_result = catch_unwind(AssertUnwindSafe(|| self.state.submit_repeating(value)));
        match callback_result {
            Ok(result) => result.map_err(map_callback_error),
            Err(callback_panic) => {
                let _ = catch_unwind(AssertUnwindSafe(|| self.state.close_and_dispose()));
                resume_unwind(callback_panic)
            }
        }
    }

    pub fn cancel(&self) {
        self.state.close_and_dispose();
    }
}

fn create_completion_state<'scope, T: 'static, E, F>(
    storage: &ScopeStorage,
    state: Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
) -> Result<Rc<CompletionState>, ReactiveError>
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

    let thunk = CallbackThunk::new_typed_fallible(callback);
    let callback = {
        let mut state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        state_ref.create_callback(thunk)?
    };
    let weak = WeakOwnerToken::from_typed(&state);
    Ok(Rc::new(CompletionState::new(
        weak,
        scheduler,
        storage.scope_id,
        callback,
    )))
}

pub(crate) fn create_completion_once<'scope, T: 'static, E, F>(
    storage: &ScopeStorage,
    state: Rc<RefCell<ScopeState<'scope>>>,
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
    storage: &ScopeStorage,
    state: Rc<RefCell<ScopeState<'scope>>>,
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
