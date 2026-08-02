//! Scope-owned completion messages for `'static` asynchronous tasks.

use crate::{
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk},
    },
    runtime::{self, ScopeId, ScopeState},
    scope::Scope,
};
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::{Rc, Weak},
};

type ErasedScopeState = RefCell<ScopeState<'static>>;

/// A weak, scope-owned destination for an asynchronous completion message.
///
/// The token contains neither a typed node handle nor a strong reference to
/// the scope. Once the scope is disposed, upgrading the token fails and
/// [`submit`](Self::submit) returns `false`.
#[derive(Clone)]
pub struct CompletionToken {
    state: Weak<ErasedScopeState>,
    scope_id: ScopeId,
    callback: RawId,
    marker: PhantomData<Rc<()>>,
}

impl CompletionToken {
    pub(crate) fn new(state: Weak<ErasedScopeState>, scope_id: ScopeId, callback: RawId) -> Self {
        Self {
            state,
            scope_id,
            callback,
            marker: PhantomData,
        }
    }

    /// Submit an owned value to the callback while its scope is active.
    pub fn submit<T>(&self, value: T) -> bool {
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

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Create a completion destination owned by this scope.
    pub fn completion<T: 'scope, F>(&self, mut callback: F) -> CompletionToken
    where
        F: FnMut(T) + 'scope,
    {
        let thunk = CallbackThunk::new(move |value: AnyValue<'scope>| {
            if let Some(value) = value.downcast::<T>() {
                callback(value);
            }
        });
        // SAFETY: the callback is stored in this scope's state and is dropped
        // by its lexical dispose path before the scope's captured values end.
        let thunk = unsafe { thunk.extend_lifetime() };
        let mut state = self
            .frame
            .state
            .try_borrow_mut()
            .expect("scope state borrowed while creating completion token");
        let callback = state.create_callback(thunk);
        let weak = Rc::downgrade(&self.frame.state);
        // SAFETY: the scheduler uses the same weak-state lifetime erasure and
        // never upgrades it after the owning scope has been deactivated.
        let weak = unsafe {
            std::mem::transmute::<Weak<RefCell<ScopeState<'run>>>, Weak<ErasedScopeState>>(weak)
        };
        CompletionToken::new(weak, self.frame.scope_id, callback)
    }
}
