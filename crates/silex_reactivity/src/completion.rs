//! Scope-owned completion destinations for asynchronous tasks.

use crate::{
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk},
    },
    runtime::{self, ScopeId, ScopeState},
    scope::{ErasedScopeState, ScopeStorage},
};
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, UnwindSafe, catch_unwind, resume_unwind},
    rc::{Rc, Weak},
};

/// Wrap a repeating callback with an explicit unwind-safety assertion.
///
/// `AssertUnwindSafe` itself only implements `FnOnce`; this adapter preserves
/// the `FnMut` contract required by repeating completion destinations.
pub fn unwind_safe<T, F>(callback: F) -> impl FnMut(T) + UnwindSafe
where
    F: FnMut(T),
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
    state: Weak<ErasedScopeState>,
    scope_id: ScopeId,
    callback: RawId,
    phase: Cell<CompletionPhase>,
}

impl CompletionState {
    fn inactive() -> Self {
        Self {
            state: Weak::new(),
            scope_id: ScopeId(0),
            callback: RawId::DANGLING,
            phase: Cell::new(CompletionPhase::Closed),
        }
    }

    fn new(state: Weak<ErasedScopeState>, scope_id: ScopeId, callback: RawId) -> Self {
        Self {
            state,
            scope_id,
            callback,
            phase: Cell::new(CompletionPhase::Active),
        }
    }

    fn current_state(&self) -> Option<Rc<RefCell<ScopeState<'_>>>> {
        let erased_state = self.state.upgrade()?;
        let current = {
            let state = erased_state.try_borrow().ok()?;
            state
                .scheduler
                .try_borrow()
                .ok()?
                .is_scope_current(self.scope_id, &self.state)
        };
        if !current {
            return None;
        }

        // SAFETY: the scheduler registry contains this exact state allocation,
        // and the caller only uses the restored lifetime while that state is
        // active. Scope disposal removes the registry entry before payloads
        // are dropped.
        Some(unsafe {
            std::mem::transmute::<Rc<ErasedScopeState>, Rc<RefCell<ScopeState<'_>>>>(erased_state)
        })
    }

    fn begin_once(&self) -> Option<Rc<RefCell<ScopeState<'_>>>> {
        if self.phase.replace(CompletionPhase::Completing) != CompletionPhase::Active {
            return None;
        }
        let state = self.current_state();
        if state.is_none() {
            self.phase.set(CompletionPhase::Closed);
        }
        state
    }

    fn close_and_dispose(&self) {
        if self.phase.replace(CompletionPhase::Closed) == CompletionPhase::Closed {
            return;
        }
        if let Some(state) = self.current_state() {
            runtime::dispose_nodes(&state, vec![self.callback]);
        }
    }

    fn submit_repeating<T: 'static>(&self, value: T) -> bool {
        if self.phase.get() != CompletionPhase::Active {
            return false;
        }
        let Some(state) = self.current_state() else {
            return false;
        };
        runtime::invoke_callback(&state, self.callback, AnyValue::new(value)).is_ok()
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
pub struct CompletionOnce<T> {
    state: Rc<CompletionState>,
    marker: PhantomData<Rc<T>>,
}

impl<T> Clone for CompletionOnce<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> Drop for CompletionOnce<T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.state) == 1 {
            drop_completion_state(&self.state);
        }
    }
}

impl<T: 'static> CompletionOnce<T> {
    pub(crate) fn inactive() -> Self {
        Self {
            state: Rc::new(CompletionState::inactive()),
            marker: PhantomData,
        }
    }

    pub fn submit(&self, value: T) -> bool {
        let Some(state) = self.state.begin_once() else {
            return false;
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
            (Ok(result), Ok(())) => result.is_ok(),
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
/// is resumed, and later submissions return `false`.
pub struct CompletionSender<T> {
    state: Rc<CompletionState>,
    marker: PhantomData<Rc<T>>,
}

impl<T> Clone for CompletionSender<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> Drop for CompletionSender<T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.state) == 1 {
            drop_completion_state(&self.state);
        }
    }
}

impl<T: 'static> CompletionSender<T> {
    pub(crate) fn inactive() -> Self {
        Self {
            state: Rc::new(CompletionState::inactive()),
            marker: PhantomData,
        }
    }

    pub fn submit(&self, value: T) -> bool {
        let callback_result = catch_unwind(AssertUnwindSafe(|| self.state.submit_repeating(value)));
        match callback_result {
            Ok(result) => result,
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

fn create_completion_state<'scope, T: 'static, F>(
    storage: &ScopeStorage,
    state: Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
) -> Rc<CompletionState>
where
    F: FnMut(T) + 'scope,
{
    let active = {
        let state = state.borrow();
        state.is_active()
    };
    if !active {
        return Rc::new(CompletionState::inactive());
    }

    let thunk = CallbackThunk::new_typed(callback);
    let callback = {
        let mut state_ref = state
            .try_borrow_mut()
            .expect("scope state borrowed while creating completion destination");
        state_ref
            .create_callback(thunk)
            .expect("scope active check must precede completion callback registration")
    };
    let weak = Rc::downgrade(&state);
    // SAFETY: the destination stores only a weak reference to the scope state;
    // the erased lifetime is restored only after the exact active registry
    // entry has been checked.
    let weak = unsafe {
        std::mem::transmute::<Weak<RefCell<ScopeState<'scope>>>, Weak<ErasedScopeState>>(weak)
    };
    Rc::new(CompletionState::new(weak, storage.scope_id, callback))
}

pub(crate) fn create_completion_once<'scope, T: 'static, F>(
    storage: &ScopeStorage,
    state: Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
) -> CompletionOnce<T>
where
    F: FnMut(T) + 'scope,
{
    CompletionOnce {
        state: create_completion_state(storage, state, callback),
        marker: PhantomData,
    }
}

pub(crate) fn create_completion_sender<'scope, T: 'static, F>(
    storage: &ScopeStorage,
    state: Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
) -> CompletionSender<T>
where
    F: FnMut(T) + 'scope,
{
    CompletionSender {
        state: create_completion_state(storage, state, callback),
        marker: PhantomData,
    }
}
