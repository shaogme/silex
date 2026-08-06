//! Operations on values, callbacks, and runtime execution scoping.

use super::{
    dispose::dispose_nodes,
    eval::{flush_if_idle, prepare_read},
    model::ScopeState,
    scheduler::{GlobalScheduler, TargetNode},
    storage::NodeStorage,
};
use crate::{
    ReactiveError, ReactiveResult,
    handle::NodeKindTag,
    internal::{RawId, value::AnyValue},
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

fn lookup_value_storage<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    reactive: bool,
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, Rc<RefCell<GlobalScheduler>>)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let storage = state_ref.value_storage(id, reactive)?;
    Ok((storage, state_ref.scheduler.clone()))
}

fn lookup_stored_storage<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, Rc<RefCell<GlobalScheduler>>)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let storage = state_ref.stored_storage(id)?;
    Ok((storage, state_ref.scheduler.clone()))
}

fn lookup_callback_storage<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, Rc<RefCell<GlobalScheduler>>)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let storage = state_ref.callback_storage(id)?;
    Ok((storage, state_ref.scheduler.clone()))
}

fn read_value<'scope, R>(
    storage: &NodeStorage<'scope>,
    scheduler: Rc<RefCell<GlobalScheduler>>,
    f: impl FnOnce(&AnyValue<'scope>) -> R,
) -> ReactiveResult<R> {
    match storage {
        NodeStorage::Value(cell) => {
            let lease = cell.try_read(scheduler)?;
            let result = f(&lease);
            drop(lease);
            Ok(result)
        }
        NodeStorage::Computation(computation) => {
            let lease = computation.value.try_read(scheduler)?;
            let value = lease.as_ref().ok_or(ReactiveError::Reentrant)?;
            let result = f(value);
            drop(lease);
            Ok(result)
        }
        NodeStorage::Callback(_) => Err(ReactiveError::WrongKind),
    }
}

pub(crate) fn with_signal<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    track: bool,
    f: impl FnOnce(&AnyValue<'scope>) -> R,
) -> ReactiveResult<R> {
    prepare_read(state, id, track)?;
    let (storage, scheduler) = lookup_value_storage(state, id, true)?;
    let result = read_value(&storage, scheduler, f)?;
    flush_if_idle(state);
    Ok(result)
}

fn commit_signal<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) -> ReactiveResult<()> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    if !state_ref
        .scheduler
        .borrow()
        .is_scope_active(state_ref.scope_id)
    {
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

pub(crate) fn update_signal<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&mut AnyValue<'scope>) -> (R, bool),
) -> ReactiveResult<R> {
    let (storage, scheduler) = lookup_value_storage(state, id, false)?;
    let NodeStorage::Value(cell) = storage.as_ref() else {
        return Err(ReactiveError::WrongKind);
    };
    let mut lease = cell.try_write(scheduler)?;
    let (result, changed) = f(&mut lease);
    drop(lease);
    if changed {
        commit_signal(state, id)?;
    }
    flush_if_idle(state);
    Ok(result)
}

pub(crate) fn with_stored<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&AnyValue<'scope>) -> R,
) -> ReactiveResult<R> {
    let (storage, scheduler) = lookup_stored_storage(state, id)?;
    let NodeStorage::Value(cell) = storage.as_ref() else {
        return Err(ReactiveError::WrongKind);
    };
    let lease = cell.try_read(scheduler)?;
    let result = f(&lease);
    drop(lease);
    flush_if_idle(state);
    Ok(result)
}

pub(crate) fn update_stored<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&mut AnyValue<'scope>) -> R,
) -> ReactiveResult<R> {
    let (storage, scheduler) = lookup_stored_storage(state, id)?;
    let NodeStorage::Value(cell) = storage.as_ref() else {
        return Err(ReactiveError::WrongKind);
    };
    let mut lease = cell.try_write(scheduler)?;
    let result = f(&mut lease);
    drop(lease);
    flush_if_idle(state);
    Ok(result)
}

pub(crate) fn invoke_callback<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    arg: AnyValue<'scope>,
) -> ReactiveResult<()> {
    let (storage, scheduler) = lookup_callback_storage(state, id)?;
    let NodeStorage::Callback(cell) = storage.as_ref() else {
        return Err(ReactiveError::WrongKind);
    };
    let mut lease = cell.try_write(scheduler)?;
    lease.call(arg);
    drop(lease);
    flush_if_idle(state);
    Ok(())
}

pub(crate) fn stop_effect<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<bool> {
    let (scheduler, target) = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let scheduler = state_ref.scheduler.clone();
        if !scheduler.borrow().is_scope_active(state_ref.scope_id) {
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

pub(crate) fn node_ref_get<'scope, T: Clone>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<Option<T>> {
    with_stored(state, id, |value| {
        unsafe { value.downcast_ref::<Option<T>>() }
            .cloned()
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

pub(crate) fn node_ref_set<'scope, T>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    value: T,
) -> ReactiveResult<()> {
    update_stored(state, id, |stored| {
        unsafe { stored.downcast_mut::<Option<T>>() }
            .map(|slot| *slot = Some(value))
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

pub(crate) fn node_ref_clear<'scope, T>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<()> {
    update_stored(state, id, |stored| {
        unsafe { stored.downcast_mut::<Option<T>>() }
            .map(|slot| *slot = None)
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

pub(crate) fn try_notify<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<()> {
    let should_flush = {
        let mut state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state_ref
            .scheduler
            .borrow()
            .is_scope_active(state_ref.scope_id)
        {
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

pub(crate) fn try_track<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<()> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    if !state_ref
        .scheduler
        .borrow()
        .is_scope_active(state_ref.scope_id)
    {
        return Err(ReactiveError::NoSuchNode);
    }
    if !state_ref.node_exists(id) {
        return Err(ReactiveError::NoSuchNode);
    }
    state_ref.track(id);
    Ok(())
}

pub(crate) fn try_track_many<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    ids: &[RawId],
) -> ReactiveResult<()> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    if !state_ref
        .scheduler
        .borrow()
        .is_scope_active(state_ref.scope_id)
    {
        return Err(ReactiveError::NoSuchNode);
    }
    if ids.iter().any(|id| !state_ref.node_exists(*id)) {
        return Err(ReactiveError::NoSuchNode);
    }
    state_ref.track_many(ids);
    Ok(())
}

pub(crate) fn with_untracked<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    f: impl FnOnce() -> R,
) -> R {
    let scheduler = state.borrow().scheduler.clone();
    let frame = super::scheduler::ObserverFrame::push(scheduler, None);
    let result = catch_unwind(AssertUnwindSafe(f));
    drop(frame);
    match result {
        Ok(value) => value,
        Err(panic) => resume_unwind(panic),
    }
}

pub(crate) fn with_batch<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    f: impl FnOnce() -> R,
) -> R {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        internal::value::AnyValue,
        runtime::{dispose_nodes, scheduler::GlobalScheduler},
        scope::ScopeStorage,
    };

    #[test]
    fn disposing_a_node_during_a_read_does_not_require_put_back() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let state = unsafe { storage.typed_state() };
        let raw = state
            .borrow_mut()
            .create_signal(AnyValue::new(7_i32))
            .expect("test scope should be active");

        let result = with_signal(&state, raw, false, |value| {
            assert_eq!(unsafe { value.downcast_ref::<i32>() }, Some(&7));
            dispose_nodes(&state, vec![raw]);
        });

        assert!(result.is_ok());
        assert!(!state.borrow().node_exists(raw));
        storage.dispose();
    }
}
