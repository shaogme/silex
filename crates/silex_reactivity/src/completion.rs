//! Scope-owned completion messages for `'static` asynchronous tasks.

use crate::{
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk},
    },
    runtime::{self, ScopeId, ScopeState},
    scope::{ErasedScopeState, ScopeStorage},
};
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::{Rc, Weak},
};

/// A weak, scope-owned destination for an asynchronous completion message.
///
/// The token contains neither a typed node handle nor a strong reference to
/// the scope. Once the scope is disposed, upgrading the token fails and
/// [`submit`](Self::submit) returns `false`.
pub struct CompletionToken<T> {
    state: Weak<ErasedScopeState>,
    scope_id: ScopeId,
    callback: RawId,
    marker: PhantomData<Rc<T>>,
}

impl<T> Clone for CompletionToken<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            scope_id: self.scope_id,
            callback: self.callback,
            marker: PhantomData,
        }
    }
}

impl<T: 'static> CompletionToken<T> {
    pub(crate) fn inactive() -> Self {
        Self {
            state: Weak::new(),
            scope_id: ScopeId(0),
            callback: RawId::DANGLING,
            marker: PhantomData,
        }
    }

    pub(crate) fn new(state: Weak<ErasedScopeState>, scope_id: ScopeId, callback: RawId) -> Self {
        Self {
            state,
            scope_id,
            callback,
            marker: PhantomData,
        }
    }

    /// Submit an owned value to the callback while its scope is active.
    pub fn submit(&self, value: T) -> bool {
        let Some(erased_state) = self.state.upgrade() else {
            return false;
        };

        if !erased_state
            .borrow()
            .scheduler
            .borrow()
            .is_scope_active(self.scope_id)
        {
            return false;
        }

        // SAFETY: the weak state is created from the callback's owning scope
        // and only upgraded while that scope still owns the callback node.
        // `CompletionToken` stores no strong state reference, so disposal
        // drops the state and makes this branch unreachable afterwards.
        let state = unsafe {
            std::mem::transmute::<Rc<ErasedScopeState>, Rc<RefCell<ScopeState<'_>>>>(erased_state)
        };
        let active = state
            .try_borrow()
            .ok()
            .is_some_and(|state| state.scheduler.borrow().is_scope_active(state.scope_id));
        if !active {
            return false;
        }
        runtime::invoke_callback(&state, self.callback, AnyValue::new(value)).is_ok()
    }
}

pub(crate) fn create_completion<'scope, T: 'static, F>(
    storage: &ScopeStorage,
    state: Rc<RefCell<ScopeState<'scope>>>,
    mut callback: F,
) -> CompletionToken<T>
where
    F: FnMut(T) + 'scope,
{
    let active = {
        let state = state.borrow();
        state.scheduler.borrow().is_scope_active(storage.scope_id)
    };
    if !active {
        return CompletionToken::inactive();
    }

    let thunk = CallbackThunk::new(move |value: AnyValue<'scope>| {
        // SAFETY: `CompletionToken<T>::submit` is the only way to submit
        // a value to this typed destination.
        if let Some(value) = unsafe { value.downcast::<T>() } {
            callback(value);
        }
    });
    let callback = {
        let mut state_ref = state
            .try_borrow_mut()
            .expect("scope state borrowed while creating completion token");
        state_ref.create_callback(thunk)
    };
    let weak = Rc::downgrade(&state);
    // SAFETY: the token only stores a weak reference to this scope's state.
    // The erased weak reference is upgraded only after the active-scope check
    // and cannot keep the state alive after lexical disposal.
    let weak = unsafe {
        std::mem::transmute::<Weak<RefCell<ScopeState<'scope>>>, Weak<ErasedScopeState>>(weak)
    };
    CompletionToken::new(weak, storage.scope_id, callback)
}
