//! Typed operations on values, callbacks, and runtime execution scoping.

use super::{
    dispose::dispose_nodes,
    eval::{EvaluationError, ReadTracking, flush_if_idle, prepare_fallible_read, prepare_read},
    model::{ScopePhase, ScopeState, StoredAccessMode},
    scheduler::{
        GlobalScheduler, ObserverFrame, TargetNode, has_runtime_boundary, validate_active_scheduler,
    },
    storage::{
        CallbackThunk, CallbackThunkError, NodeStorage, ReadLease, TypedNodeRef, TypedSlot,
        WriteLease,
    },
};
use crate::{
    CallbackInvokeError, CallbackInvokeResult, ReactiveError, ReactiveResult,
    borrow::SharedCell,
    error::{ErrorContext, ErrorHandlerRef, ErrorSlotRef, HandlerError, HandlerLease},
    handle::NodeKindTag,
    internal::NodeId,
    unsafe_boundary::{ActiveOwnerProof, ScopedPtr, restore_cleanup_stored_slot},
};
use std::{
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

fn value_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    reactive: bool,
    validate_runtime: bool,
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, SharedCell<GlobalScheduler>)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let storage = state_ref.value_storage(id, reactive)?;
    if validate_runtime || has_runtime_boundary()? {
        validate_active_scheduler(&state_ref.scheduler)?;
    }
    Ok((storage, state_ref.scheduler.clone()))
}

pub(crate) struct PreparedSignalRead {
    scheduler: SharedCell<GlobalScheduler>,
}

fn prepare_value_access<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    kind: Option<NodeKindTag>,
    validate_runtime: bool,
) -> ReactiveResult<SharedCell<GlobalScheduler>> {
    let (_storage, scheduler) = value_scheduler(state, id, true, validate_runtime)?;
    let proof = ActiveOwnerProof::from_state(state)?;
    match kind {
        Some(kind) => {
            proof.restore_typed_slot(state, id, kind, value.pointer())?;
        }
        None => {
            proof.restore_value_slot(state, id, value.pointer())?;
        }
    }
    Ok(scheduler)
}

fn prepare_signal_read<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    tracking: ReadTracking,
    kind: Option<NodeKindTag>,
) -> ReactiveResult<PreparedSignalRead> {
    prepare_read(state, id, tracking)?;
    let scheduler = prepare_value_access(
        state,
        id,
        value,
        kind,
        matches!(tracking, ReadTracking::Tracked),
    )?;
    Ok(PreparedSignalRead { scheduler })
}

fn stored_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
) -> ReactiveResult<(
    Rc<NodeStorage<'scope>>,
    SharedCell<GlobalScheduler>,
    StoredAccessMode,
)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let (storage, mode) = state_ref.stored_value_storage(id)?;
    validate_active_scheduler(&state_ref.scheduler)?;
    Ok((storage, state_ref.scheduler.clone(), mode))
}

fn node_ref_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, SharedCell<GlobalScheduler>)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let storage = state_ref.node_ref_storage(id)?;
    validate_active_scheduler(&state_ref.scheduler)?;
    Ok((storage, state_ref.scheduler.clone()))
}

pub(crate) fn invoke_error_handler<'scope, E>(
    handler: &ErrorHandlerRef<'scope, E>,
    error: E,
) -> Result<(), HandlerError>
where
    E: 'scope,
{
    let state = handler.storage().owner_token().state();
    let owner = {
        let state_ref = state
            .try_borrow()
            .map_err(|error| HandlerError::new(error, ErrorContext::new("handler state lookup")))?;
        let context = ErrorContext::new("handler registry lookup").with_owner(state_ref.owner_id.0);
        if state_ref.phase == ScopePhase::Released {
            return Err(HandlerError::scope_released(context));
        }
        let entry = state_ref
            .error_handlers
            .get(handler.key())
            .ok_or_else(|| HandlerError::generation_mismatch(context))?;
        if entry.identity != handler.record() {
            return Err(HandlerError::generation_mismatch(context));
        }
        if !entry.owner.is_active() {
            return Err(HandlerError::inactive(context));
        }
        state_ref.owner_id.0
    };
    let proof = ActiveOwnerProof::from_state(&state)
        .map_err(|error| HandlerError::new(error, ErrorContext::new("handler proof")))?;
    let record = proof
        .restore_handler_record(&state, handler.key(), handler.record().cast())
        .map_err(|error| HandlerError::new(error, ErrorContext::new("handler record")))?;
    record.call(
        error,
        ErrorContext::new("handler callback").with_owner(owner),
        false,
    )
}

pub(crate) fn acquire_error_handler_lease<'scope, E>(
    handler: &ErrorHandlerRef<'scope, E>,
) -> Result<HandlerLease<'scope, E>, HandlerError>
where
    E: 'scope,
{
    let state = handler.storage().owner_token().state();
    let owner = {
        let state_ref = state
            .try_borrow()
            .map_err(|error| HandlerError::new(error, ErrorContext::new("handler state lookup")))?;
        let context = ErrorContext::new("handler lease").with_owner(state_ref.owner_id.0);
        if state_ref.phase == ScopePhase::Released {
            return Err(HandlerError::scope_released(context));
        }
        let entry = state_ref
            .error_handlers
            .get(handler.key())
            .ok_or_else(|| HandlerError::generation_mismatch(context))?;
        if entry.identity != handler.record() {
            return Err(HandlerError::generation_mismatch(context));
        }
        entry.owner.add_lease(context)?;
        entry.owner.clone()
    };
    let proof = ActiveOwnerProof::from_state(&state)
        .map_err(|error| HandlerError::new(error, ErrorContext::new("handler proof")))?;
    let record = proof
        .clone_handler_record(&state, handler.key(), handler.record().cast())
        .map_err(|error| HandlerError::new(error, ErrorContext::new("handler record")))?;
    Ok(record.lease(owner, record.clone()))
}

fn restore_read_slot<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    slot: TypedNodeRef<'scope, T>,
    kind: Option<NodeKindTag>,
    cleanup: bool,
    scheduler: SharedCell<GlobalScheduler>,
) -> ReactiveResult<ReadLease<'scope, T>> {
    let slot = if cleanup {
        restore_cleanup_stored_slot(state, id, slot.pointer())?
    } else {
        let proof = ActiveOwnerProof::from_state(state)?;
        match kind {
            Some(kind) => proof.restore_typed_slot(state, id, kind, slot.pointer())?,
            None => proof.restore_value_slot(state, id, slot.pointer())?,
        }
    };
    slot.try_read(scheduler)?.into_initialized()
}

fn read_typed<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    slot: TypedNodeRef<'scope, T>,
    scheduler: SharedCell<GlobalScheduler>,
    kind: Option<NodeKindTag>,
    cleanup: bool,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    let lease = restore_read_slot(state, id, slot, kind, cleanup, scheduler)?;
    let result = f(&lease);
    drop(lease);
    Ok(result)
}

fn read_signal_lease_with_tracking<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    tracking: ReadTracking,
) -> ReactiveResult<ReadLease<'scope, T>> {
    let prepared = prepare_signal_read(state, id, value, tracking, None)?;
    restore_read_slot(state, id, value, None, false, prepared.scheduler)
}

pub(crate) fn read_signal_lease<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
) -> ReactiveResult<ReadLease<'scope, T>> {
    read_signal_lease_with_tracking(state, id, value, ReadTracking::Tracked)
}

pub(crate) fn read_signal_lease_untracked<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
) -> ReactiveResult<ReadLease<'scope, T>> {
    read_signal_lease_with_tracking(state, id, value, ReadTracking::Untracked)
}

pub(crate) fn read_fallible_signal_lease<'scope, T, E>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    errors: ErrorSlotRef<'scope, E>,
) -> CallbackInvokeResult<ReadLease<'scope, T>, E>
where
    E: 'scope,
{
    let prepared = prepare_fallible_signal(state, id, value, errors, ReadTracking::Tracked)?;
    restore_read_slot(
        state,
        id,
        value,
        Some(NodeKindTag::Computed),
        false,
        prepared.scheduler,
    )
    .map_err(CallbackInvokeError::Runtime)
}

pub(crate) fn read_fallible_signal_lease_untracked<'scope, T, E>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    errors: ErrorSlotRef<'scope, E>,
) -> CallbackInvokeResult<ReadLease<'scope, T>, E>
where
    E: 'scope,
{
    let prepared = prepare_fallible_signal(state, id, value, errors, ReadTracking::Untracked)?;
    restore_read_slot(
        state,
        id,
        value,
        Some(NodeKindTag::Computed),
        false,
        prepared.scheduler,
    )
    .map_err(CallbackInvokeError::Runtime)
}

pub(crate) fn write_signal_lease<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
) -> ReactiveResult<WriteLease<'scope, T>> {
    let (_storage, scheduler) = value_scheduler(state, id, false, true)?;
    let proof = ActiveOwnerProof::from_state(state)?;
    proof
        .restore_typed_slot(state, id, NodeKindTag::Signal, value.pointer())?
        .try_write(scheduler)?
        .into_initialized()
}

pub(crate) fn read_stored_lease<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
) -> ReactiveResult<(ReadLease<'scope, T>, bool)> {
    let (_storage, scheduler, mode) = stored_scheduler(state, id)?;
    let lease = restore_read_slot(
        state,
        id,
        value,
        Some(NodeKindTag::Stored),
        mode == StoredAccessMode::RunningCleanup,
        scheduler,
    )?;
    Ok((lease, mode == StoredAccessMode::Active))
}

pub(crate) fn write_stored_lease<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
) -> ReactiveResult<(WriteLease<'scope, T>, bool)> {
    let (_storage, scheduler, mode) = stored_scheduler(state, id)?;
    let slot = if mode == StoredAccessMode::RunningCleanup {
        restore_cleanup_stored_slot(state, id, value.pointer())?
    } else {
        let proof = ActiveOwnerProof::from_state(state)?;
        proof.restore_typed_slot(state, id, NodeKindTag::Stored, value.pointer())?
    };
    let lease = slot.try_write(scheduler)?.into_initialized()?;
    Ok((lease, mode == StoredAccessMode::Active))
}

pub(crate) fn with_signal<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    let prepared = prepare_signal_read(state, id, value, ReadTracking::Tracked, None)?;
    let result = read_typed(state, id, value, prepared.scheduler, None, false, f)?;
    flush_if_idle(state)?;
    Ok(result)
}

pub(crate) fn with_signal_untracked<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    let prepared = prepare_signal_read(state, id, value, ReadTracking::Untracked, None)?;
    let result = read_typed(state, id, value, prepared.scheduler, None, false, f)?;
    flush_if_idle(state)?;
    Ok(result)
}

pub(crate) fn track_signal<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
) -> ReactiveResult<()> {
    let _prepared = prepare_signal_read(state, id, value, ReadTracking::Tracked, None)?;
    flush_if_idle(state)
}

fn map_evaluation_error<'scope, E>(
    state: &ScopeState<'scope>,
    id: NodeId,
    errors: ErrorSlotRef<'scope, E>,
    error: EvaluationError<'scope>,
) -> CallbackInvokeError<E>
where
    E: 'scope,
{
    match error {
        EvaluationError::Runtime(error) => CallbackInvokeError::Runtime(error),
        EvaluationError::User => {
            let proof = match ActiveOwnerProof::from_state(state) {
                Ok(proof) => proof,
                Err(error) => return CallbackInvokeError::Runtime(error),
            };
            let slot = match proof.restore_error_slot(state, id, errors.pointer()) {
                Ok(slot) => slot,
                Err(error) => return CallbackInvokeError::Runtime(error),
            };
            CallbackInvokeError::User(match slot.take() {
                Ok(error) => error,
                Err(error) => return CallbackInvokeError::Runtime(error),
            })
        }
        EvaluationError::Callback(_) => {
            CallbackInvokeError::Runtime(ReactiveError::InvariantViolation)
        }
        EvaluationError::Handler(error) => CallbackInvokeError::Handler(error),
    }
}

fn prepare_fallible_signal<'scope, T, E>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    errors: ErrorSlotRef<'scope, E>,
    tracking: ReadTracking,
) -> CallbackInvokeResult<PreparedSignalRead, E>
where
    E: 'scope,
{
    prepare_fallible_read(state, id, tracking)
        .map_err(|error| map_evaluation_error(state, id, errors, error))?;
    let scheduler = prepare_value_access(
        state,
        id,
        value,
        Some(NodeKindTag::Computed),
        matches!(tracking, ReadTracking::Tracked),
    )
    .map_err(CallbackInvokeError::Runtime)?;
    Ok(PreparedSignalRead { scheduler })
}

pub(crate) fn track_fallible_signal<'scope, T, E>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    errors: ErrorSlotRef<'scope, E>,
) -> CallbackInvokeResult<(), E>
where
    E: 'scope,
{
    let _prepared = prepare_fallible_signal(state, id, value, errors, ReadTracking::Tracked)?;
    flush_if_idle(state).map_err(CallbackInvokeError::Runtime)
}

pub(crate) fn with_fallible_signal<'scope, T, E, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    errors: ErrorSlotRef<'scope, E>,
    f: impl FnOnce(&T) -> Result<R, ReactiveError>,
) -> CallbackInvokeResult<R, E>
where
    E: 'scope,
{
    let prepared = prepare_fallible_signal(state, id, value, errors, ReadTracking::Tracked)?;
    let result = match read_typed(
        state,
        id,
        value,
        prepared.scheduler,
        Some(NodeKindTag::Computed),
        false,
        f,
    ) {
        Ok(result) => result,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    flush_if_idle(state).map_err(CallbackInvokeError::Runtime)?;
    result.map_err(CallbackInvokeError::Runtime)
}

pub(crate) fn with_fallible_signal_untracked<'scope, T, E, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    errors: ErrorSlotRef<'scope, E>,
    f: impl FnOnce(&T) -> Result<R, ReactiveError>,
) -> CallbackInvokeResult<R, E>
where
    E: 'scope,
{
    let prepared = prepare_fallible_signal(state, id, value, errors, ReadTracking::Untracked)?;
    let result = match read_typed(
        state,
        id,
        value,
        prepared.scheduler,
        Some(NodeKindTag::Computed),
        false,
        f,
    ) {
        Ok(result) => result,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    flush_if_idle(state).map_err(CallbackInvokeError::Runtime)?;
    result.map_err(CallbackInvokeError::Runtime)
}

pub(crate) fn commit_signal<'scope>(state: &ScopeState<'scope>, id: NodeId) -> ReactiveResult<()> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    if !state_ref.is_active()? {
        return Ok(());
    }
    let epoch = state_ref
        .scheduler
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?
        .next_epoch();
    let Some(node) = state_ref.nodes.get_mut(id) else {
        return Ok(());
    };
    node.updated_epoch = epoch;
    node.last_computed_epoch = epoch;
    node.version = node.version.wrapping_add(1);
    state_ref.queue_dependents(id)?;
    Ok(())
}

pub(crate) fn update_signal<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&mut T) -> (R, bool),
) -> ReactiveResult<R> {
    let (_storage, scheduler) = value_scheduler(state, id, false, true)?;
    let proof = ActiveOwnerProof::from_state(state)?;
    let slot = proof.restore_typed_slot(state, id, NodeKindTag::Signal, value.pointer())?;
    let mut lease = slot.try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    let (result, changed) = f(stored);
    drop(lease);
    if changed {
        commit_signal(state, id)?;
    }
    flush_if_idle(state)?;
    Ok(result)
}

pub(crate) fn notify<'scope>(state: &ScopeState<'scope>, id: NodeId) -> ReactiveResult<()> {
    let scheduler = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?
        .scheduler
        .clone();
    validate_active_scheduler(&scheduler)?;
    let should_flush = {
        let mut state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state_ref.is_active()? {
            return Err(ReactiveError::NoSuchNode);
        }
        if !state_ref.node_exists(id) {
            return Err(ReactiveError::NoSuchNode);
        }
        if state_ref.mark_notified(id)? {
            state_ref.queue_dependents(id)?;
        }
        state_ref
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .should_flush()
    };
    if should_flush {
        flush_if_idle(state)?;
    }
    Ok(())
}

pub(crate) fn with_stored<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    let (_storage, scheduler, mode) = stored_scheduler(state, id)?;
    let result = read_typed(
        state,
        id,
        value,
        scheduler,
        Some(NodeKindTag::Stored),
        mode == StoredAccessMode::RunningCleanup,
        f,
    )?;
    if mode == StoredAccessMode::Active {
        flush_if_idle(state)?;
    }
    Ok(result)
}

pub(crate) fn track_stored<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
) -> ReactiveResult<()> {
    let (_storage, _scheduler, mode) = stored_scheduler(state, id)?;
    if mode == StoredAccessMode::RunningCleanup {
        restore_cleanup_stored_slot(state, id, value.pointer())?;
    } else {
        let proof = ActiveOwnerProof::from_state(state)?;
        proof.restore_typed_slot(state, id, NodeKindTag::Stored, value.pointer())?;
    }
    if mode == StoredAccessMode::Active {
        flush_if_idle(state)?;
    }
    Ok(())
}

pub(crate) fn update_stored<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&mut T) -> R,
) -> ReactiveResult<R> {
    let (_storage, scheduler, mode) = stored_scheduler(state, id)?;
    let mut lease = if mode == StoredAccessMode::RunningCleanup {
        restore_cleanup_stored_slot(state, id, value.pointer())?.try_write(scheduler)?
    } else {
        let proof = ActiveOwnerProof::from_state(state)?;
        proof
            .restore_typed_slot(state, id, NodeKindTag::Stored, value.pointer())?
            .try_write(scheduler)?
    };
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    let result = f(stored);
    drop(lease);
    if mode == StoredAccessMode::Active {
        flush_if_idle(state)?;
    }
    Ok(result)
}

pub(crate) fn node_ref_get<'scope, T: Clone>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, Option<T>>,
) -> ReactiveResult<Option<T>> {
    let (_storage, scheduler) = node_ref_scheduler(state, id)?;
    read_typed(
        state,
        id,
        value,
        scheduler,
        Some(NodeKindTag::NodeRef),
        false,
        Clone::clone,
    )
}

pub(crate) fn node_ref_set<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    slot: TypedNodeRef<'scope, Option<T>>,
    value: T,
) -> ReactiveResult<()> {
    let (_storage, scheduler) = node_ref_scheduler(state, id)?;
    let proof = ActiveOwnerProof::from_state(state)?;
    let slot = proof.restore_typed_slot(state, id, NodeKindTag::NodeRef, slot.pointer())?;
    let mut lease = slot.try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    *stored = Some(value);
    drop(lease);
    flush_if_idle(state)?;
    Ok(())
}

pub(crate) fn node_ref_clear<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    slot: TypedNodeRef<'scope, Option<T>>,
) -> ReactiveResult<()> {
    let (_storage, scheduler) = node_ref_scheduler(state, id)?;
    let proof = ActiveOwnerProof::from_state(state)?;
    let slot = proof.restore_typed_slot(state, id, NodeKindTag::NodeRef, slot.pointer())?;
    let mut lease = slot.try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    *stored = None;
    drop(lease);
    flush_if_idle(state)?;
    Ok(())
}

pub(crate) fn invoke_callback<'scope, T, E>(
    state: &ScopeState<'scope>,
    id: NodeId,
    callback_pointer: ScopedPtr<TypedSlot<CallbackThunk<'scope, T, E>>>,
    arg: T,
) -> Result<(), CallbackThunkError<E>>
where
    T: 'scope,
    E: 'scope,
{
    let (scheduler, _storage) = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| CallbackThunkError::Runtime(ReactiveError::BorrowConflict))?;
        let storage = state_ref
            .callback_storage(id)
            .map_err(CallbackThunkError::Runtime)?;
        let scheduler = state_ref.scheduler.clone();
        validate_active_scheduler(&scheduler).map_err(CallbackThunkError::Runtime)?;
        (scheduler, storage)
    };
    let proof = ActiveOwnerProof::from_state(state).map_err(CallbackThunkError::Runtime)?;
    let callback_slot = proof
        .restore_typed_slot(state, id, NodeKindTag::Callback, callback_pointer)
        .map_err(CallbackThunkError::Runtime)?;
    let mut lease = callback_slot
        .try_write(scheduler.clone())
        .map_err(CallbackThunkError::Runtime)?;
    let mut callback = match mem::take(&mut *lease) {
        Some(callback) => callback,
        None => {
            let active = state
                .try_borrow()
                .map_err(|_| CallbackThunkError::Runtime(ReactiveError::BorrowConflict))?
                .is_active()
                .map_err(CallbackThunkError::Runtime)?;
            return Err(CallbackThunkError::Runtime(if active {
                ReactiveError::BorrowConflict
            } else {
                ReactiveError::NoSuchNode
            }));
        }
    };
    drop(lease);

    let result = catch_unwind(AssertUnwindSafe(|| callback.call(arg)));
    let should_restore = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| CallbackThunkError::Runtime(ReactiveError::BorrowConflict))?;
        state_ref.is_active().map_err(CallbackThunkError::Runtime)? && state_ref.node_exists(id)
    };
    let restore_result = if should_restore {
        match callback_slot.try_write(scheduler) {
            Ok(mut lease) => {
                if lease.is_some() {
                    Err(ReactiveError::BorrowConflict)
                } else {
                    *lease = Some(callback);
                    Ok(())
                }
            }
            Err(error) => Err(error),
        }
    } else {
        Ok(())
    };

    let result = match result {
        Ok(result) => {
            restore_result.map_err(CallbackThunkError::Runtime)?;
            result.map_err(CallbackThunkError::User)
        }
        Err(panic) => resume_unwind(panic),
    };
    flush_if_idle(state).map_err(CallbackThunkError::Runtime)?;
    result
}

pub(crate) fn stop_effect<'scope>(state: &ScopeState<'scope>, id: NodeId) -> ReactiveResult<bool> {
    let (scheduler, target) = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let scheduler = state_ref.scheduler.clone();
        if !state_ref.is_active()? {
            return Ok(false);
        }
        let Some(node) = state_ref.nodes.get(id) else {
            return Ok(false);
        };
        if node.kind != NodeKindTag::Effect {
            return Err(ReactiveError::WrongKind);
        }
        (
            scheduler,
            TargetNode {
                owner_id: state_ref.owner_id,
                node: id,
            },
        )
    };

    scheduler
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?
        .cancel_effect(target);
    let dispose_result = catch_unwind(AssertUnwindSafe(|| dispose_nodes(state, vec![id])));
    let flush_result = catch_unwind(AssertUnwindSafe(|| flush_if_idle(state)));
    match (dispose_result, flush_result) {
        (Err(panic), _) => resume_unwind(panic),
        (Ok(_), Err(panic)) => resume_unwind(panic),
        (Ok(Err(error)), _) => return Err(error),
        (Ok(Ok(outcome)), Ok(Ok(()))) => {
            if let Some(error) = outcome.handler_errors.into_iter().next() {
                return Err(ReactiveError::Handler(error));
            }
        }
        (Ok(Ok(_)), Ok(Err(error))) => return Err(error),
    }
    Ok(true)
}

pub(crate) fn with_untracked<'scope, R>(
    state: &ScopeState<'scope>,
    f: impl FnOnce() -> R,
) -> ReactiveResult<R> {
    let scheduler = state.try_borrow()?.scheduler.clone();
    let frame = ObserverFrame::push_untracked(scheduler)?;
    let result = catch_unwind(AssertUnwindSafe(f));
    drop(frame);
    match result {
        Ok(value) => Ok(value),
        Err(panic) => resume_unwind(panic),
    }
}

pub(crate) fn with_runtime<'scope, R>(
    state: &ScopeState<'scope>,
    f: impl FnOnce() -> R,
) -> ReactiveResult<R> {
    let scheduler = state.try_borrow()?.scheduler.clone();
    let frame = ObserverFrame::push_runtime_boundary(scheduler)?;
    let result = catch_unwind(AssertUnwindSafe(f));
    drop(frame);
    match result {
        Ok(value) => Ok(value),
        Err(panic) => resume_unwind(panic),
    }
}

pub(crate) fn with_batch<'scope, R>(
    state: &ScopeState<'scope>,
    f: impl FnOnce() -> R,
) -> ReactiveResult<R> {
    let scheduler = state.try_borrow()?.scheduler.clone();
    let mut scheduler_ref = scheduler
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    scheduler_ref.batch_depth = scheduler_ref.batch_depth.saturating_add(1);
    drop(scheduler_ref);
    let result = catch_unwind(AssertUnwindSafe(f));
    {
        let mut sched = scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        sched.batch_depth = sched.batch_depth.saturating_sub(1);
    }
    let flush_result = catch_unwind(AssertUnwindSafe(|| flush_if_idle(state)));
    match result {
        Ok(value) => match flush_result {
            Ok(Ok(())) => Ok(value),
            Ok(Err(error)) => Err(error),
            Err(panic) => resume_unwind(panic),
        },
        Err(panic) => resume_unwind(panic),
    }
}
