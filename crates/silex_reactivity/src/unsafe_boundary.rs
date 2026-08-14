//! The single owner boundary for non-`'static` runtime state.
//!
//! `ScopeState<'scope>` contains callbacks and payloads borrowed from the
//! lexical owner. The scheduler and asynchronous destinations must be able to
//! keep weak references to that state without making those references public
//! or requiring every caller to repeat the lifetime proof. This module is the
//! only place that converts the erased representation back to a typed one.
//!
//! `OwnerToken<'scope>` is the proof object used by the rest of the runtime.
//! It is created either from a lexical `ScopeStorage` owner or from a registry
//! entry that has already passed the scheduler's scope-id and pointer check.
//! The token keeps the state allocation alive for the duration of the typed
//! operation; it does not extend the lifetime of payloads beyond the owner.

use crate::runtime::ScopeState;
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::{Rc, Weak},
};

pub(crate) type ErasedScopeState = RefCell<ScopeState<'static>>;

/// A typed capability to access one owner while its lexical lifetime is live.
pub(crate) struct OwnerToken<'scope> {
    state: Rc<ErasedScopeState>,
    marker: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<'scope> Clone for OwnerToken<'scope> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            marker: PhantomData,
        }
    }
}

impl<'scope> OwnerToken<'scope> {
    pub(crate) fn from_storage(
        state: Rc<ErasedScopeState>,
        _owner: PhantomData<fn(&'scope ()) -> &'scope ()>,
    ) -> Self {
        Self {
            state,
            marker: PhantomData,
        }
    }

    /// Return the state with the lifetime carried by this owner token.
    pub(crate) fn state(&self) -> Rc<RefCell<ScopeState<'scope>>> {
        restore_state(self.state.clone())
    }
}

/// A weak owner reference shared by the scheduler and async destinations.
#[derive(Clone)]
pub(crate) struct WeakOwnerToken {
    state: Weak<ErasedScopeState>,
}

impl WeakOwnerToken {
    pub(crate) fn from_typed<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>) -> Self {
        let erased = erase_state(state.clone());
        Self {
            state: Rc::downgrade(&erased),
        }
    }

    pub(crate) fn upgrade<'scope>(&self) -> Option<OwnerToken<'scope>> {
        self.state
            .upgrade()
            .map(|state| OwnerToken::from_storage(state, PhantomData))
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.state, &other.state)
    }
}

/// Erase only the lifetime parameter while retaining the exact `Rc` identity.
fn erase_state<'scope>(state: Rc<RefCell<ScopeState<'scope>>>) -> Rc<ErasedScopeState> {
    // SAFETY: `OwnerToken` is the only API that can restore this representation.
    // The owner controls disposal, and all weak users are gated by the scheduler
    // registry before they can create a typed token.
    unsafe { std::mem::transmute(state) }
}

/// Restore a state lifetime under the proof carried by `OwnerToken`.
fn restore_state<'scope>(state: Rc<ErasedScopeState>) -> Rc<RefCell<ScopeState<'scope>>> {
    // SAFETY: callers can obtain a typed state only through `OwnerToken`; its
    // lifetime is supplied by the lexical owner or by a validated registry
    // lookup. Disposal removes registry access before owner payloads are dropped.
    unsafe { std::mem::transmute(state) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{internal::value::AnyValue, runtime::GlobalScheduler, scope::ScopeStorage};

    fn store_borrowed<'scope>(
        storage: &ScopeStorage,
        value: &'scope str,
    ) -> Rc<RefCell<ScopeState<'scope>>> {
        let state = storage.owner_token(PhantomData).state();
        state
            .borrow_mut()
            .create_stored(AnyValue::new(value))
            .expect("owner token should preserve the lexical payload lifetime");
        state
    }

    #[test]
    fn owner_token_supports_non_static_payloads_until_disposal() {
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler);
        let value = String::from("borrowed");
        let state = store_borrowed(&storage, value.as_str());

        assert_eq!(state.borrow().nodes.len(), 1);
        storage.dispose_untracked();
        assert!(state.borrow().nodes.is_empty());
    }
}
