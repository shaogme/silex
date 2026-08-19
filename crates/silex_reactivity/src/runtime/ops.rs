//! Typed operations on values, callbacks, and runtime execution scoping.

use super::{
    dispose::dispose_nodes,
    eval::{EvaluationError, flush_if_idle, prepare_fallible_read, prepare_read},
    model::{ScopePhase, ScopeState, StoredAccessMode},
    scheduler::{GlobalScheduler, ObserverFrame, TargetNode, validate_active_scheduler},
    storage::{CallbackThunk, CallbackThunkError, NodeStorage, TypedNodeRef},
};
use crate::{
    CallbackInvokeError, CallbackInvokeResult, ReactiveError, ReactiveResult,
    error::{ErrorContext, ErrorHandlerRef, ErrorSlotRef, HandlerError, HandlerLease},
    handle::NodeKindTag,
    internal::NodeId,
};
use std::{
    cell::RefCell,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

fn value_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    reactive: bool,
    validate_runtime: bool,
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, Rc<RefCell<GlobalScheduler>>)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let storage = state_ref.value_storage(id, reactive)?;
    if validate_runtime {
        validate_active_scheduler(&state_ref.scheduler)?;
    }
    Ok((storage, state_ref.scheduler.clone()))
}

fn stored_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
) -> ReactiveResult<(
    Rc<NodeStorage<'scope>>,
    Rc<RefCell<GlobalScheduler>>,
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
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, Rc<RefCell<GlobalScheduler>>)> {
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
    let record = unsafe { handler.restore_record() };
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
    let record = unsafe { handler.restore_record() };
    Ok(record.lease(owner, handler.record()))
}

fn read_typed<'scope, T, R>(
    slot: TypedNodeRef<'scope, T>,
    scheduler: Rc<RefCell<GlobalScheduler>>,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    // SAFETY: callers validate the owning node before restoring this pointer.
    let lease = unsafe { slot.restore() }.try_read(scheduler)?;
    let value = lease.as_ref().ok_or(ReactiveError::NoSuchNode)?;
    let result = f(value);
    drop(lease);
    Ok(result)
}

pub(crate) fn with_signal<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    track: bool,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    prepare_read(state, id, track)?;
    let (_storage, scheduler) = value_scheduler(state, id, true, track)?;
    let result = read_typed(value, scheduler, f)?;
    flush_if_idle(state)?;
    Ok(result)
}

pub(crate) fn with_fallible_signal<'scope, T, E, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    errors: ErrorSlotRef<'scope, E>,
    track: bool,
    f: impl FnOnce(&T) -> Result<R, ReactiveError>,
) -> CallbackInvokeResult<R, E>
where
    E: 'scope,
{
    if let Err(error) = prepare_fallible_read(state, id, track) {
        return Err(match error {
            EvaluationError::Runtime(error) => CallbackInvokeError::Runtime(error),
            EvaluationError::User => {
                // SAFETY: the failed read validated the live computed node.
                CallbackInvokeError::User(unsafe { errors.restore() }.take())
            }
            EvaluationError::Callback(_) => {
                CallbackInvokeError::Runtime(ReactiveError::InvariantViolation)
            }
            EvaluationError::Handler(error) => CallbackInvokeError::Handler(error),
        });
    }
    let (_storage, scheduler) = match value_scheduler(state, id, true, track) {
        Ok(scheduler) => scheduler,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    let result = match read_typed(value, scheduler, f) {
        Ok(result) => result,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    flush_if_idle(state).map_err(CallbackInvokeError::Runtime)?;
    result.map_err(CallbackInvokeError::Runtime)
}

fn commit_signal<'scope>(state: &ScopeState<'scope>, id: NodeId) -> ReactiveResult<()> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    if !state_ref.is_active() {
        return Ok(());
    }
    let epoch = state_ref.scheduler.borrow_mut().next_epoch();
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
    // SAFETY: `value_scheduler` validated the live signal node.
    let mut lease = unsafe { value.restore() }.try_write(scheduler)?;
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
        if !state_ref.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        if !state_ref.node_exists(id) {
            return Err(ReactiveError::NoSuchNode);
        }
        if state_ref.mark_notified(id) {
            state_ref.queue_dependents(id)?;
        }
        state_ref.scheduler.borrow().should_flush()
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
    let result = read_typed(value, scheduler, f)?;
    if mode == StoredAccessMode::Active {
        flush_if_idle(state)?;
    }
    Ok(result)
}

pub(crate) fn update_stored<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: NodeId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&mut T) -> R,
) -> ReactiveResult<R> {
    let (_storage, scheduler, mode) = stored_scheduler(state, id)?;
    // SAFETY: `stored_scheduler` validated the live stored node.
    let mut lease = unsafe { value.restore() }.try_write(scheduler)?;
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
    read_typed(value, scheduler, Clone::clone)
}

pub(crate) fn node_ref_set<'scope, T>(
    state: &ScopeState<'scope>,
    id: NodeId,
    slot: TypedNodeRef<'scope, Option<T>>,
    value: T,
) -> ReactiveResult<()> {
    let (_storage, scheduler) = node_ref_scheduler(state, id)?;
    // SAFETY: `node_ref_scheduler` validated the live node-ref node.
    let mut lease = unsafe { slot.restore() }.try_write(scheduler)?;
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
    // SAFETY: `node_ref_scheduler` validated the live node-ref node.
    let mut lease = unsafe { slot.restore() }.try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    *stored = None;
    drop(lease);
    flush_if_idle(state)?;
    Ok(())
}

pub(crate) fn invoke_callback<'scope, T, E>(
    state: &ScopeState<'scope>,
    id: NodeId,
    callback_ref: TypedNodeRef<'scope, CallbackThunk<'scope, T, E>>,
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
    // SAFETY: `callback_storage` validated the live callback node.
    let callback_slot = unsafe { callback_ref.restore() };
    let mut lease = callback_slot
        .try_write(scheduler.clone())
        .map_err(CallbackThunkError::Runtime)?;
    let mut callback = match mem::take(&mut *lease) {
        Some(callback) => callback,
        None => {
            let active = state
                .try_borrow()
                .map_err(|_| CallbackThunkError::Runtime(ReactiveError::BorrowConflict))?
                .try_is_active()
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
        state_ref
            .try_is_active()
            .map_err(CallbackThunkError::Runtime)?
            && state_ref.node_exists(id)
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
        if !state_ref.is_active() {
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

pub(crate) fn with_untracked<'scope, R>(state: &ScopeState<'scope>, f: impl FnOnce() -> R) -> R {
    let scheduler = state.borrow().scheduler.clone();
    let frame = ObserverFrame::push_untracked(scheduler);
    let result = catch_unwind(AssertUnwindSafe(f));
    drop(frame);
    match result {
        Ok(value) => value,
        Err(panic) => resume_unwind(panic),
    }
}

pub(crate) fn with_batch<'scope, R>(
    state: &ScopeState<'scope>,
    f: impl FnOnce() -> R,
) -> ReactiveResult<R> {
    let scheduler = state.try_borrow()?.scheduler.clone();
    scheduler
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?
        .batch_depth += 1;
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
