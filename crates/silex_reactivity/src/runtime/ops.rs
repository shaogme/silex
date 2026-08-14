//! Typed operations on values, callbacks, and runtime execution scoping.

use super::{
    dispose::dispose_nodes,
    eval::{EvaluationError, flush_if_idle, prepare_fallible_read, prepare_read},
    model::{ScopeState, StoredAccessMode},
    scheduler::{GlobalScheduler, ObserverFrame, TargetNode},
    storage::{CallbackThunk, CallbackThunkError, TypedNodeRef},
};
use crate::{
    CallbackInvokeError, CallbackInvokeResult, ReactiveError, ReactiveResult,
    error::{ErrorHandlerCall, ErrorHandlerKey, ErrorSlot},
    handle::NodeKindTag,
    internal::RawId,
    scope::ScopeStorage,
};
use std::{
    cell::RefCell,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

fn value_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: RawId,
    reactive: bool,
) -> ReactiveResult<Rc<RefCell<GlobalScheduler>>> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let _ = state_ref.value_storage(id, reactive)?;
    Ok(state_ref.scheduler.clone())
}

fn stored_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: RawId,
) -> ReactiveResult<(Rc<RefCell<GlobalScheduler>>, StoredAccessMode)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let (_, mode) = state_ref.stored_value_storage(id)?;
    Ok((state_ref.scheduler.clone(), mode))
}

fn node_ref_scheduler<'scope>(
    state: &ScopeState<'scope>,
    id: RawId,
) -> ReactiveResult<Rc<RefCell<GlobalScheduler>>> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let _ = state_ref.node_ref_storage(id)?;
    Ok(state_ref.scheduler.clone())
}

pub(crate) fn invoke_error_handler<'scope, E>(
    storage: &ScopeStorage,
    key: ErrorHandlerKey,
    callback: &'scope dyn ErrorHandlerCall<E>,
    error: E,
) -> ReactiveResult<()>
where
    E: 'scope,
{
    let state = storage.owner_token(PhantomData).state();
    {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if state_ref.error_handlers.get(key).is_none() {
            return Err(ReactiveError::NoSuchNode);
        }
    }
    callback.call(error);
    Ok(())
}

fn read_typed<'scope, T, R>(
    slot: TypedNodeRef<'scope, T>,
    scheduler: Rc<RefCell<GlobalScheduler>>,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    let lease = slot.slot().try_read(scheduler)?;
    let value = lease.as_ref().ok_or(ReactiveError::NoSuchNode)?;
    let result = f(value);
    drop(lease);
    Ok(result)
}

pub(crate) fn with_signal<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: RawId,
    value: TypedNodeRef<'scope, T>,
    track: bool,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    prepare_read(state, id, track)?;
    let scheduler = value_scheduler(state, id, true)?;
    let result = read_typed(value, scheduler, f)?;
    flush_if_idle(state);
    Ok(result)
}

pub(crate) fn with_fallible_signal<'scope, T, E, R>(
    state: &ScopeState<'scope>,
    id: RawId,
    value: TypedNodeRef<'scope, T>,
    errors: &'scope ErrorSlot<E>,
    track: bool,
    f: impl FnOnce(&T) -> Result<R, ReactiveError>,
) -> CallbackInvokeResult<R, E>
where
    E: 'scope,
{
    if let Err(error) = prepare_fallible_read(state, id, track) {
        return Err(match error {
            EvaluationError::Runtime(error) => CallbackInvokeError::Runtime(error),
            EvaluationError::User => CallbackInvokeError::User(errors.take()),
            EvaluationError::Callback(_) => {
                CallbackInvokeError::Runtime(ReactiveError::TypeMismatch)
            }
        });
    }
    let scheduler = match value_scheduler(state, id, true) {
        Ok(scheduler) => scheduler,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    let result = match read_typed(value, scheduler, f) {
        Ok(result) => result,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    flush_if_idle(state);
    result.map_err(CallbackInvokeError::Runtime)
}

fn commit_signal<'scope>(state: &ScopeState<'scope>, id: RawId) -> ReactiveResult<()> {
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
    state_ref.queue_dependents(id);
    Ok(())
}

pub(crate) fn update_signal<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: RawId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&mut T) -> (R, bool),
) -> ReactiveResult<R> {
    let scheduler = value_scheduler(state, id, false)?;
    let mut lease = value.slot().try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    let (result, changed) = f(stored);
    drop(lease);
    if changed {
        commit_signal(state, id)?;
    }
    flush_if_idle(state);
    Ok(result)
}

pub(crate) fn with_stored<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: RawId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    let (scheduler, mode) = stored_scheduler(state, id)?;
    let result = read_typed(value, scheduler, f)?;
    if mode == StoredAccessMode::Active {
        flush_if_idle(state);
    }
    Ok(result)
}

pub(crate) fn update_stored<'scope, T, R>(
    state: &ScopeState<'scope>,
    id: RawId,
    value: TypedNodeRef<'scope, T>,
    f: impl FnOnce(&mut T) -> R,
) -> ReactiveResult<R> {
    let (scheduler, mode) = stored_scheduler(state, id)?;
    let mut lease = value.slot().try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    let result = f(stored);
    drop(lease);
    if mode == StoredAccessMode::Active {
        flush_if_idle(state);
    }
    Ok(result)
}

pub(crate) fn node_ref_get<'scope, T: Clone>(
    state: &ScopeState<'scope>,
    id: RawId,
    value: TypedNodeRef<'scope, Option<T>>,
) -> ReactiveResult<Option<T>> {
    let scheduler = node_ref_scheduler(state, id)?;
    read_typed(value, scheduler, Clone::clone)
}

pub(crate) fn node_ref_set<'scope, T>(
    state: &ScopeState<'scope>,
    id: RawId,
    slot: TypedNodeRef<'scope, Option<T>>,
    value: T,
) -> ReactiveResult<()> {
    let scheduler = node_ref_scheduler(state, id)?;
    let mut lease = slot.slot().try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    *stored = Some(value);
    drop(lease);
    flush_if_idle(state);
    Ok(())
}

pub(crate) fn node_ref_clear<'scope, T>(
    state: &ScopeState<'scope>,
    id: RawId,
    slot: TypedNodeRef<'scope, Option<T>>,
) -> ReactiveResult<()> {
    let scheduler = node_ref_scheduler(state, id)?;
    let mut lease = slot.slot().try_write(scheduler)?;
    let stored = lease.as_mut().ok_or(ReactiveError::NoSuchNode)?;
    *stored = None;
    drop(lease);
    flush_if_idle(state);
    Ok(())
}

pub(crate) fn invoke_callback<'scope, T, E>(
    state: &ScopeState<'scope>,
    id: RawId,
    callback: TypedNodeRef<'scope, CallbackThunk<'scope, T, E>>,
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
        (state_ref.scheduler.clone(), storage)
    };
    let mut lease = callback
        .slot()
        .try_write(scheduler)
        .map_err(CallbackThunkError::Runtime)?;
    let callback = lease
        .as_mut()
        .ok_or(CallbackThunkError::Runtime(ReactiveError::NoSuchNode))?;
    let result = callback.call(arg).map_err(CallbackThunkError::User);
    drop(lease);
    flush_if_idle(state);
    result
}

pub(crate) fn stop_effect<'scope>(state: &ScopeState<'scope>, id: RawId) -> ReactiveResult<bool> {
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
                scope_id: state_ref.scope_id,
                node: id,
            },
        )
    };

    scheduler.borrow_mut().cancel_effect(target);
    let dispose_result = catch_unwind(AssertUnwindSafe(|| dispose_nodes(state, vec![id])));
    let flush_result = catch_unwind(AssertUnwindSafe(|| flush_if_idle(state)));
    match (dispose_result, flush_result) {
        (Err(panic), _) => resume_unwind(panic),
        (Ok(()), Err(panic)) => resume_unwind(panic),
        (Ok(()), Ok(())) => {}
    }
    Ok(true)
}

pub(crate) fn notify<'scope>(state: &ScopeState<'scope>, id: RawId) -> ReactiveResult<()> {
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
            state_ref.queue_dependents(id);
        }
        state_ref.scheduler.borrow().should_flush()
    };
    if should_flush {
        flush_if_idle(state);
    }
    Ok(())
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

pub(crate) fn with_batch<'scope, R>(state: &ScopeState<'scope>, f: impl FnOnce() -> R) -> R {
    let scheduler = state.borrow().scheduler.clone();
    scheduler.borrow_mut().batch_depth += 1;
    let result = catch_unwind(AssertUnwindSafe(f));
    {
        let mut sched = scheduler.borrow_mut();
        sched.batch_depth = sched.batch_depth.saturating_sub(1);
    }
    flush_if_idle(state);
    match result {
        Ok(value) => value,
        Err(panic) => resume_unwind(panic),
    }
}
