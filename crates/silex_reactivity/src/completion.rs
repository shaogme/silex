//! Scope-owned completion messages for `'static` asynchronous tasks.

use crate::{
    child::Scope,
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk},
    },
    runtime::{self, ScopeId, ScopeState},
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

impl<'scope, 'run> Scope<'scope, 'run> {
    /// Create a completion destination owned by this scope.
    pub fn completion<T: 'static, F>(&self, callback: F) -> CompletionToken<T>
    where
        F: FnMut(T) + 'static,
    {
        // SAFETY: a `'static` callback cannot borrow data that ends before the
        // scope's disposal path.
        unsafe { self.completion_scoped(callback) }
    }

    /// Create a completion destination with a callback borrowed from this scope.
    ///
    /// # Safety
    ///
    /// The callback must not borrow data that can be dropped before this scope
    /// is disposed. In particular, references to locals created inside the
    /// surrounding `Runtime::run` or `Scope::child` callback are not allowed;
    /// move scope-owned handles and owned values into the callback instead.
    pub unsafe fn completion_scoped<T: 'static, F>(&self, mut callback: F) -> CompletionToken<T>
    where
        F: FnMut(T) + 'scope,
    {
        let thunk = CallbackThunk::new(move |value: AnyValue<'scope>| {
            // SAFETY: `CompletionToken<T>::submit` is the only way to submit
            // a value to this typed destination.
            if let Some(value) = unsafe { value.downcast::<T>() } {
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
