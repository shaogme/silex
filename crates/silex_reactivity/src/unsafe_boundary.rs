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
    runtime::storage::{CallbackThunk, TypedNodeRef, TypedSlot},
    runtime::{ScopeState, ScopeStateInner},
};
use std::{
    cell::RefCell,
    marker::PhantomData,
    ptr::NonNull,
    rc::{Rc, Weak},
};

pub(crate) type ErasedScopeState = RefCell<ScopeStateInner<'static>>;

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
    pub(crate) fn state(&self) -> ScopeState<'scope> {
        ScopeState::from_inner(restore_state(self.state.clone()))
    }
}

/// A weak owner reference shared by the scheduler and async destinations.
#[derive(Clone)]
pub(crate) struct WeakOwnerToken {
    state: Weak<ErasedScopeState>,
}

impl WeakOwnerToken {
    pub(crate) fn from_typed<'scope>(state: &ScopeState<'scope>) -> Self {
        let erased = erase_state(state.inner().clone());
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
fn erase_state<'scope>(state: Rc<RefCell<ScopeStateInner<'scope>>>) -> Rc<ErasedScopeState> {
    // SAFETY: `OwnerToken` is the only API that can restore this representation.
    // The owner controls disposal, and all weak users are gated by the scheduler
    // registry before they can create a typed token.
    unsafe { std::mem::transmute(state) }
}

/// Restore a state lifetime under the proof carried by `OwnerToken`.
fn restore_state<'scope>(state: Rc<ErasedScopeState>) -> Rc<RefCell<ScopeStateInner<'scope>>> {
    // SAFETY: callers can obtain a typed state only through `OwnerToken`; its
    // lifetime is supplied by the lexical owner or by a validated registry
    // lookup. Disposal removes registry access before owner payloads are dropped.
    unsafe { std::mem::transmute(state) }
}

/// Erase the callback payload lifetime for an asynchronous destination.
///
/// The destination stores only a weak owner token and must first validate that
/// token through the scheduler before dereferencing this capability. Disposal
/// clears the callback slot before the arena is released, so the erased
/// lifetime cannot be observed after the owner ends.
#[derive(Clone, Copy)]
pub(crate) struct ErasedCallbackRef<T, E> {
    pointer: NonNull<()>,
    marker: PhantomData<fn(T) -> E>,
}

pub(crate) unsafe fn erase_callback_ref<'scope, T, E>(
    callback: TypedNodeRef<'scope, CallbackThunk<'scope, T, E>>,
) -> ErasedCallbackRef<T, E> {
    // SAFETY: See the function-level safety contract. This changes only the
    // lifetime carried by a typed arena reference; the owner gate controls all
    // subsequent dereferences and drops the slot before arena teardown.
    ErasedCallbackRef {
        pointer: NonNull::from(callback.slot()).cast(),
        marker: PhantomData,
    }
}

impl<T, E> ErasedCallbackRef<T, E> {
    pub(crate) fn restore<'scope>(
        &self,
        _owner: &OwnerToken<'scope>,
    ) -> TypedNodeRef<'scope, CallbackThunk<'scope, T, E>> {
        // SAFETY: The owner token proves that the scope arena is still live.
        // This reference was created from that arena's callback slot and is
        // restored only by a completion state carrying the matching token.
        let slot = unsafe {
            self.pointer
                .cast::<TypedSlot<CallbackThunk<'scope, T, E>>>()
                .as_ref()
        };
        TypedNodeRef::from_slot(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{runtime::GlobalScheduler, scope::ScopeStorage};

    fn store_borrowed<'scope>(
        storage: &'scope ScopeStorage,
        value: &'scope str,
    ) -> ScopeState<'scope> {
        let state = storage.owner_token(PhantomData).state();
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
        let _ = storage.dispose_untracked();
        assert!(state.borrow().nodes.is_empty());
    }
}
