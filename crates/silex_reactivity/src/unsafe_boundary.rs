//! Proof-producing boundary for erased owner state and scoped pointers.
//!
//! This is the only module that performs lifetime restoration, raw pointer
//! dereferencing, or `Rc` reconstruction for the reactivity runtime. Callers
//! must first obtain an owner proof and then use one of the proof methods that
//! validates the owner generation, scope phase, node kind, and payload identity
//! in the same operation.

use crate::{
    ReactiveError, ReactiveResult,
    borrow::SharedCell,
    error::{ErrorHandlerKey, ErrorSlot, HandlerRecord},
    handle::NodeKindTag,
    internal::NodeId,
    owner::ScopeStorage,
    runtime::storage::TypedSlot,
    runtime::{OwnerId, ScopePhase, ScopeState, ScopeStateInner},
};
use std::{
    marker::PhantomData,
    ptr::NonNull,
    rc::{Rc, Weak},
};

pub(crate) type ErasedScopeState = crate::borrow::BorrowCell<ScopeStateInner<'static>>;

/// A pointer whose provenance is captured from a live Rust reference.
///
/// The pointer is intentionally opaque outside this module. It can be copied
/// to preserve the public handle `Copy` contract, but it cannot be dereferenced
/// without an owner proof.
#[derive(PartialEq, Eq)]
pub(crate) struct ScopedPtr<T> {
    pointer: NonNull<T>,
}

impl<T> Copy for ScopedPtr<T> {}

impl<T> Clone for ScopedPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> ScopedPtr<T> {
    pub(crate) fn from_ref(reference: &T) -> Self {
        Self {
            pointer: NonNull::from(reference),
        }
    }

    pub(crate) fn from_rc(value: &Rc<T>) -> Self {
        let pointer = Rc::as_ptr(value).cast_mut();
        Self {
            // SAFETY: `Rc::as_ptr` is non-null for every live allocation.
            pointer: unsafe { NonNull::new_unchecked(pointer) },
        }
    }

    pub(crate) fn cast<U>(self) -> ScopedPtr<U> {
        ScopedPtr {
            pointer: self.pointer.cast(),
        }
    }

    fn identity(&self) -> ScopedPtr<()> {
        ScopedPtr {
            pointer: self.pointer.cast(),
        }
    }
}

/// A typed capability to access one owner while its lexical lifetime is live.
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
            // state and every node payload allocation for that lexical owner.
            state: ScopeState::from_inner(restore_state(storage.state.clone())),
            marker: PhantomData,
        }
    }

    fn from_typed(state: &ScopeState<'scope>) -> Self {
        Self {
            state: state.clone(),
            marker: PhantomData,
        }
    }

    fn from_erased(state: Rc<ErasedScopeState>) -> Self {
        Self {
            // SAFETY: `from_erased` is called only after the registry proof
            // checks the exact weak identity, owner generation, and phase.
            state: ScopeState::from_inner(restore_state(state)),
            marker: PhantomData,
        }
    }

    pub(crate) fn state(&self) -> ScopeState<'scope> {
        self.state.clone()
    }
}

/// A proof that an owner is still active and its typed payloads may be used.
#[derive(Clone)]
pub(crate) struct ActiveOwnerProof<'scope> {
    owner: OwnerToken<'scope>,
    owner_id: OwnerId,
}

impl<'scope> ActiveOwnerProof<'scope> {
    /// Build an active proof from a typed lexical state after checking the
    /// scheduler registry and exact weak-state identity.
    pub(crate) fn from_state(state: &ScopeState<'scope>) -> ReactiveResult<Self> {
        let (owner_id, scheduler, phase) = {
            let state_ref = state.try_borrow()?;
            (
                state_ref.owner_id,
                state_ref.scheduler.clone(),
                state_ref.phase,
            )
        };
        if phase != ScopePhase::Active {
            return Err(ReactiveError::NoSuchNode);
        }
        let weak = WeakOwnerToken::from_typed(state);
        let current = scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .is_scope_current(owner_id, &weak);
        if !current {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(Self {
            owner: OwnerToken::from_typed(state),
            owner_id,
        })
    }

    /// Build a proof from a scheduler registry entry. The erased lifetime is
    /// restored only after all registry and state checks have succeeded.
    pub(crate) fn from_registry(
        id: OwnerId,
        generation: u64,
        expected: &WeakOwnerToken,
        state: Rc<ErasedScopeState>,
    ) -> ReactiveResult<Option<Self>> {
        if id.1 != generation || !WeakOwnerToken::from_erased(state.clone()).ptr_eq(expected) {
            return Ok(None);
        }
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if state_ref.owner_id != id || state_ref.phase != ScopePhase::Active {
            return Ok(None);
        }
        drop(state_ref);
        Ok(Some(Self {
            owner: OwnerToken::from_erased(state),
            owner_id: id,
        }))
    }

    pub(crate) fn state(&self) -> ScopeState<'scope> {
        self.owner.state()
    }

    pub(crate) fn restore_typed_slot<T>(
        &self,
        state: &ScopeState<'scope>,
        node: NodeId,
        kind: NodeKindTag,
        pointer: ScopedPtr<TypedSlot<T>>,
    ) -> ReactiveResult<&'scope TypedSlot<T>> {
        self.validate_payload(state, node, kind, pointer.identity())?;
        // SAFETY: `validate_payload` checked the owner identity, active phase,
        // node generation/kind, and exact payload address immediately above.
        Ok(unsafe { pointer.pointer.as_ref() })
    }

    pub(crate) fn restore_value_slot<T>(
        &self,
        state: &ScopeState<'scope>,
        node: NodeId,
        pointer: ScopedPtr<TypedSlot<T>>,
    ) -> ReactiveResult<&'scope TypedSlot<T>> {
        self.validate_owner_state(state)?;
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let node_ref = state_ref.nodes.get(node).ok_or(ReactiveError::NoSuchNode)?;
        if !matches!(node_ref.kind, NodeKindTag::Signal | NodeKindTag::Computed) {
            return Err(ReactiveError::WrongKind);
        }
        let data = state_ref.data.get(node).ok_or(ReactiveError::NoSuchNode)?;
        if data.storage.payload_identity() != Some(pointer.identity()) {
            return Err(ReactiveError::NoSuchNode);
        }
        drop(state_ref);
        // SAFETY: the owner, phase, node kind, and exact payload identity were
        // checked while the proof still held the matching typed state.
        Ok(unsafe { pointer.pointer.as_ref() })
    }

    pub(crate) fn restore_error_slot<E>(
        &self,
        state: &ScopeState<'scope>,
        node: NodeId,
        pointer: ScopedPtr<ErrorSlot<E>>,
    ) -> ReactiveResult<&'scope ErrorSlot<E>> {
        self.validate_error_payload(state, node, pointer.identity())?;
        // SAFETY: `validate_error_payload` proved that this is the live error
        // slot retained by the current computed node.
        Ok(unsafe { pointer.pointer.as_ref() })
    }

    pub(crate) fn restore_handler_record<E>(
        &self,
        state: &ScopeState<'scope>,
        key: ErrorHandlerKey,
        pointer: ScopedPtr<HandlerRecord<'scope, E>>,
    ) -> ReactiveResult<&'scope HandlerRecord<'scope, E>> {
        self.validate_owner_state(state)?;
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let entry = state_ref
            .error_handlers
            .get(key)
            .ok_or(ReactiveError::NoSuchNode)?;
        if entry.identity != pointer.identity() {
            return Err(ReactiveError::NoSuchNode);
        }
        drop(state_ref);
        // SAFETY: the registry identity check proves that the record allocation
        // is live for this active owner generation.
        Ok(unsafe { pointer.pointer.as_ref() })
    }

    pub(crate) fn clone_handler_record<E: 'scope>(
        &self,
        state: &ScopeState<'scope>,
        key: ErrorHandlerKey,
        pointer: ScopedPtr<HandlerRecord<'scope, E>>,
    ) -> ReactiveResult<Rc<HandlerRecord<'scope, E>>> {
        self.restore_handler_record(state, key, pointer)?;
        // SAFETY: the preceding registry validation proves this pointer is an
        // allocation owned by an existing `Rc<HandlerRecord>`.
        unsafe { Rc::increment_strong_count(pointer.pointer.as_ptr()) };
        // SAFETY: exactly one strong reference was added immediately above.
        Ok(unsafe { Rc::from_raw(pointer.pointer.as_ptr()) })
    }

    fn validate_payload(
        &self,
        state: &ScopeState<'scope>,
        node: NodeId,
        kind: NodeKindTag,
        pointer: ScopedPtr<()>,
    ) -> ReactiveResult<()> {
        self.validate_owner_state(state)?;
        let state_ref = state.try_borrow()?;
        let node_ref = state_ref.nodes.get(node).ok_or(ReactiveError::NoSuchNode)?;
        if node_ref.kind != kind {
            return Err(ReactiveError::WrongKind);
        }
        let data = state_ref.data.get(node).ok_or(ReactiveError::NoSuchNode)?;
        if data.storage.payload_identity() != Some(pointer) {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(())
    }

    fn validate_error_payload(
        &self,
        state: &ScopeState<'scope>,
        node: NodeId,
        pointer: ScopedPtr<()>,
    ) -> ReactiveResult<()> {
        self.validate_owner_state(state)?;
        let state_ref = state.try_borrow()?;
        let node_ref = state_ref.nodes.get(node).ok_or(ReactiveError::NoSuchNode)?;
        if node_ref.kind != NodeKindTag::Computed {
            return Err(ReactiveError::WrongKind);
        }
        let data = state_ref.data.get(node).ok_or(ReactiveError::NoSuchNode)?;
        if data.storage.error_slot_identity() != Some(pointer) {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(())
    }

    fn validate_owner_state(&self, state: &ScopeState<'scope>) -> ReactiveResult<()> {
        if !Rc::ptr_eq(self.owner.state().inner(), state.inner()) {
            return Err(ReactiveError::RuntimeMismatch);
        }
        let state_ref = state.try_borrow()?;
        if state_ref.owner_id != self.owner_id || state_ref.phase != ScopePhase::Active {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(())
    }
}

/// Restore a stored payload during the one explicitly supported cleanup phase.
pub(crate) fn restore_cleanup_stored_slot<'scope, T>(
    state: &ScopeState<'scope>,
    node: NodeId,
    pointer: ScopedPtr<TypedSlot<T>>,
) -> ReactiveResult<&'scope TypedSlot<T>> {
    let state_ref = state.try_borrow()?;
    if state_ref.phase != ScopePhase::RunningCleanup {
        return Err(ReactiveError::NoSuchNode);
    }
    if state_ref
        .nodes
        .get(node)
        .is_none_or(|node_ref| node_ref.kind != NodeKindTag::Stored)
    {
        return Err(ReactiveError::WrongKind);
    }
    let data = state_ref.data.get(node).ok_or(ReactiveError::NoSuchNode)?;
    if data.storage.payload_identity() != Some(pointer.identity()) {
        return Err(ReactiveError::NoSuchNode);
    }
    drop(state_ref);
    // SAFETY: cleanup phase, node kind, and exact payload identity were
    // validated before accessing the stable Box allocation.
    Ok(unsafe { pointer.pointer.as_ref() })
}

/// A proof used only while removing graph edges from an owner that is closing.
#[derive(Clone)]
pub(crate) struct CleanupOwnerProof<'scope> {
    owner: OwnerToken<'scope>,
}

impl<'scope> CleanupOwnerProof<'scope> {
    pub(crate) fn from_registry(
        id: OwnerId,
        generation: u64,
        expected: &WeakOwnerToken,
        state: Rc<ErasedScopeState>,
    ) -> ReactiveResult<Option<Self>> {
        if id.1 != generation || !WeakOwnerToken::from_erased(state.clone()).ptr_eq(expected) {
            return Ok(None);
        }
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if state_ref.owner_id != id || state_ref.phase == ScopePhase::Released {
            return Ok(None);
        }
        drop(state_ref);
        Ok(Some(Self {
            owner: OwnerToken::from_erased(state),
        }))
    }

    pub(crate) fn state(&self) -> ScopeState<'scope> {
        self.owner.state()
    }
}

/// Reconstruct a typed owner state from an erased allocation.
fn restore_state<'scope>(state: Rc<ErasedScopeState>) -> SharedCell<ScopeStateInner<'scope>> {
    // SAFETY: this function is private to the proof boundary. Every call is
    // either tied to a lexical `ScopeStorage` borrow or follows a complete
    // active/cleanup registry proof above.
    unsafe { std::mem::transmute(state) }
}

/// A weak owner reference shared by the scheduler and asynchronous destinations.
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

    pub(crate) fn upgrade_erased(&self) -> Option<Rc<ErasedScopeState>> {
        self.state.upgrade()
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.state, &other.state)
    }
}

fn erase_state<'scope>(state: SharedCell<ScopeStateInner<'scope>>) -> Rc<ErasedScopeState> {
    // SAFETY: only this module can erase and later restore the owner lifetime.
    unsafe { std::mem::transmute(state) }
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
    use crate::{owner::ScopeStorage, runtime::GlobalScheduler};

    fn store_borrowed<'scope>(
        storage: &'scope ScopeStorage,
        value: &'scope str,
    ) -> ScopeState<'scope> {
        let state = storage.owner_token().state();
        let slot = storage.alloc_slot(value);
        state
            .try_borrow_mut()
            .expect("state write")
            .create_stored(slot)
            .expect("owner token should preserve the lexical payload lifetime");
        state
    }

    #[test]
    fn owner_token_supports_non_static_payloads_until_disposal() {
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler).expect("test owner setup");
        let value = String::from("borrowed");
        let state = store_borrowed(&storage, value.as_str());

        assert_eq!(state.try_borrow().expect("state read").nodes.len(), 1);
        let outcome = storage.dispose_untracked();
        assert!(outcome.released);
        assert!(outcome.error.is_none());
        assert!(state.try_borrow().expect("state read").nodes.is_empty());
    }

    #[test]
    fn active_and_cleanup_proofs_respect_scope_phase() {
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler).expect("test owner setup");
        let state = storage.owner_token().state();
        let active = ActiveOwnerProof::from_state(&state).expect("active proof");
        drop(active);

        state
            .try_borrow_mut()
            .expect("state write")
            .begin_quiescing()
            .expect("owner should begin closing");
        assert!(matches!(
            ActiveOwnerProof::from_state(&state),
            Err(ReactiveError::NoSuchNode)
        ));

        let expected = WeakOwnerToken::from_typed(&state);
        let cleanup = CleanupOwnerProof::from_registry(
            storage.owner_id,
            storage.owner_id.1,
            &expected,
            storage.state.clone(),
        )
        .expect("cleanup proof lookup")
        .expect("closing owner should retain cleanup proof");
        assert_eq!(
            cleanup.state().try_borrow().expect("state read").phase,
            ScopePhase::Quiescing
        );
        drop(cleanup);

        let outcome = storage.dispose_untracked();
        assert!(outcome.released);
        assert!(matches!(
            ActiveOwnerProof::from_state(&state),
            Err(ReactiveError::NoSuchNode)
        ));
    }

    #[test]
    fn payload_identity_mismatch_is_rejected_before_restore() {
        let scheduler = GlobalScheduler::new();
        let storage = ScopeStorage::new(scheduler).expect("test owner setup");
        let state = storage.owner_token().state();
        let first = state
            .try_borrow_mut()
            .expect("state write")
            .create_signal(storage.alloc_slot(1_i32))
            .expect("first signal");
        let second = state
            .try_borrow_mut()
            .expect("state write")
            .create_signal(storage.alloc_slot(2_i32))
            .expect("second signal");
        let first_pointer = state
            .try_borrow()
            .expect("state read")
            .typed_node_ref::<i32>(first)
            .expect("first pointer")
            .pointer();
        let second_pointer = state
            .try_borrow()
            .expect("state read")
            .typed_node_ref::<i32>(second)
            .expect("second pointer")
            .pointer();
        let proof = ActiveOwnerProof::from_state(&state).expect("active proof");

        assert!(matches!(
            proof.restore_value_slot(&state, first, second_pointer),
            Err(ReactiveError::NoSuchNode)
        ));
        assert!(
            proof
                .restore_value_slot(&state, first, first_pointer)
                .is_ok()
        );
        let outcome = storage.dispose_untracked();
        assert!(outcome.released);
    }
}
