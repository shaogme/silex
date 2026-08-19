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

use crate::{
    owner::ScopeStorage,
    runtime::{ScopeState, ScopeStateInner},
};
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::{Rc, Weak},
};

pub(crate) type ErasedScopeState = RefCell<ScopeStateInner<'static>>;

/// A typed capability to access one owner while its lexical lifetime is live.
///
/// The token is never created from an owner id.  Lexical callers create it
/// from a borrowed [`ScopeStorage`], while the scheduler creates it only after
/// its owner-generation and weak-identity checks have succeeded.
pub(crate) struct OwnerToken<'scope> {
    state: ScopeState<'scope>,
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
    pub(crate) fn from_storage(storage: &'scope ScopeStorage) -> Self {
        Self {
            // SAFETY: `storage` is borrowed for `'scope` and owns the erased
            // state as well as the bump arena that contains every payload.
            // The owner close path clears all typed slots before the storage
            // can be dropped, so this token cannot outlive its payload arena.
            state: ScopeState::from_inner(unsafe { restore_state(storage.state.clone()) }),
            marker: PhantomData,
        }
    }

    /// Restore a typed owner after the scheduler has validated its runtime,
    /// owner generation, weak identity, and active phase.
    ///
    /// # Safety
    ///
    /// The caller must have performed those checks immediately before calling
    /// this function and must not expose the returned token after the owner
    /// enters its closing phase.  The owner close order is: deactivate the
    /// registry slot, clear node payloads, detach graph edges, then release
    /// the slot generation.
    pub(crate) unsafe fn from_validated(state: Rc<ErasedScopeState>) -> Self {
        Self {
            // SAFETY: upheld by the function contract above.
            state: ScopeState::from_inner(unsafe { restore_state(state) }),
            marker: PhantomData,
        }
    }

    /// Return the state with the lifetime carried by this owner token.
    pub(crate) fn state(&self) -> ScopeState<'scope> {
        self.state.clone()
    }
}

/// A weak owner reference shared by the scheduler and async destinations.
#[derive(Clone)]
pub(crate) struct WeakOwnerToken {
    state: Weak<ErasedScopeState>,
}

impl WeakOwnerToken {
    pub(crate) fn from_erased(state: Rc<ErasedScopeState>) -> Self {
        Self {
            state: Rc::downgrade(&state),
        }
    }

    pub(crate) fn from_typed<'scope>(state: &ScopeState<'scope>) -> Self {
        let erased = erase_state(state.inner().clone());
        Self {
            state: Rc::downgrade(&erased),
        }
    }

    /// Upgrade only to the erased allocation.  A typed owner is restored by
    /// the runtime validator after it has checked identity and phase.
    pub(crate) fn upgrade_erased(&self) -> Option<Rc<ErasedScopeState>> {
        self.state.upgrade()
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.state, &other.state)
    }
}

/// Erase only the lifetime parameter while retaining the exact `Rc` identity.
fn erase_state<'scope>(state: Rc<RefCell<ScopeStateInner<'scope>>>) -> Rc<ErasedScopeState> {
    // SAFETY: `OwnerToken` is the only API that can restore this representation.
    // The owner controls disposal, and all weak users are gated by the scheduler
    // registry before they can create a typed token.
    unsafe { std::mem::transmute(state) }
}

/// Restore a state lifetime under the proof carried by `OwnerToken`.
unsafe fn restore_state<'scope>(
    state: Rc<ErasedScopeState>,
) -> Rc<RefCell<ScopeStateInner<'scope>>> {
    // SAFETY: callers are limited to `OwnerToken::from_storage` and
    // `OwnerToken::from_validated`; both paths document the lexical or
    // scheduler proof that keeps the owner arena alive during the operation.
    unsafe { std::mem::transmute(state) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{owner::ScopeStorage, runtime::GlobalScheduler};

    fn store_borrowed<'scope>(
        storage: &'scope ScopeStorage,
        value: &'scope str,
    ) -> ScopeState<'scope> {
        let state = storage.owner_token().state();
        let slot = storage.alloc_slot(value);
        state
            .borrow_mut()
            .create_stored(slot)
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
        let outcome = storage.dispose_untracked();
        assert!(outcome.released);
        assert!(outcome.error.is_none());
        assert!(state.borrow().nodes.is_empty());
    }
}
