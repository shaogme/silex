//! Operations on values, callbacks, and runtime execution scoping.

use super::{
    dispose::dispose_nodes,
    eval::{EvaluationError, flush_if_idle, prepare_fallible_read, prepare_read},
    model::{ScopeState, StoredAccessMode},
    scheduler::{GlobalScheduler, TargetNode},
    storage::NodeStorage,
};
use crate::{
    CallbackInvokeError, CallbackInvokeResult, ReactiveError, ReactiveResult,
    error::ErrorHandlerKey,
    handle::NodeKindTag,
    internal::{
        RawId,
        value::{AnyValue, CallbackThunkError},
    },
    scope::ScopeStorage,
};
use std::{
    cell::RefCell,
    marker::PhantomData,
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

fn lookup_stored_value_storage<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<(
    Rc<NodeStorage<'scope>>,
    Rc<RefCell<GlobalScheduler>>,
    StoredAccessMode,
)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let (storage, mode) = state_ref.stored_value_storage(id)?;
    Ok((storage, state_ref.scheduler.clone(), mode))
}

fn lookup_node_ref_storage<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<(Rc<NodeStorage<'scope>>, Rc<RefCell<GlobalScheduler>>)> {
    let state_ref = state
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    let storage = state_ref.node_ref_storage(id)?;
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

pub(crate) fn invoke_error_handler<'scope, E>(
    storage: &ScopeStorage,
    key: ErrorHandlerKey,
    error: E,
) -> ReactiveResult<()>
where
    E: 'scope,
{
    let state: Rc<RefCell<ScopeState<'scope>>> = storage.owner_token(PhantomData).state();
    let callback = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        state_ref
            .error_handlers
            .get(key)
            .map(|entry| entry.callback.clone())
            .ok_or(ReactiveError::NoSuchNode)?
    };
    callback(AnyValue::new(error));
    Ok(())
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

pub(crate) fn with_fallible_signal<'scope, R, E>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    track: bool,
    f: impl FnOnce(&AnyValue<'scope>) -> Result<R, ReactiveError>,
) -> CallbackInvokeResult<R, E>
where
    E: 'scope,
{
    if let Err(error) = prepare_fallible_read(state, id, track) {
        return Err(match error {
            EvaluationError::Runtime(error) => CallbackInvokeError::Runtime(error),
            EvaluationError::User(value) => unsafe {
                value
                    .downcast::<E>()
                    .map(CallbackInvokeError::User)
                    .unwrap_or(CallbackInvokeError::Runtime(ReactiveError::TypeMismatch))
            },
            EvaluationError::Callback(_) => {
                CallbackInvokeError::Runtime(ReactiveError::TypeMismatch)
            }
        });
    }
    let (storage, scheduler) = match lookup_value_storage(state, id, true) {
        Ok(value) => value,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    let result = match read_value(&storage, scheduler, f) {
        Ok(result) => result,
        Err(error) => return Err(CallbackInvokeError::Runtime(error)),
    };
    flush_if_idle(state);
    result.map_err(CallbackInvokeError::Runtime)
}

fn commit_signal<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) -> ReactiveResult<()> {
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
    let (storage, scheduler, mode) = lookup_stored_value_storage(state, id)?;
    let NodeStorage::Value(cell) = storage.as_ref() else {
        return Err(ReactiveError::WrongKind);
    };
    let lease = cell.try_read(scheduler)?;
    let result = f(&lease);
    drop(lease);
    if mode == StoredAccessMode::Active {
        flush_if_idle(state);
    }
    Ok(result)
}

pub(crate) fn update_stored<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&mut AnyValue<'scope>) -> R,
) -> ReactiveResult<R> {
    let (storage, scheduler, mode) = lookup_stored_value_storage(state, id)?;
    let NodeStorage::Value(cell) = storage.as_ref() else {
        return Err(ReactiveError::WrongKind);
    };
    let mut lease = cell.try_write(scheduler)?;
    let result = f(&mut lease);
    drop(lease);
    if mode == StoredAccessMode::Active {
        flush_if_idle(state);
    }
    Ok(result)
}

fn with_node_ref<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&AnyValue<'scope>) -> R,
) -> ReactiveResult<R> {
    let (storage, scheduler) = lookup_node_ref_storage(state, id)?;
    let NodeStorage::Value(cell) = storage.as_ref() else {
        return Err(ReactiveError::WrongKind);
    };
    let lease = cell.try_read(scheduler)?;
    let result = f(&lease);
    drop(lease);
    flush_if_idle(state);
    Ok(result)
}

fn update_node_ref<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&mut AnyValue<'scope>) -> R,
) -> ReactiveResult<R> {
    let (storage, scheduler) = lookup_node_ref_storage(state, id)?;
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
) -> Result<(), CallbackThunkError<'scope>> {
    let (storage, scheduler) =
        lookup_callback_storage(state, id).map_err(CallbackThunkError::Runtime)?;
    let NodeStorage::Callback(cell) = storage.as_ref() else {
        return Err(CallbackThunkError::Runtime(ReactiveError::WrongKind));
    };
    let result = {
        let mut lease = cell
            .try_write(scheduler)
            .map_err(CallbackThunkError::Runtime)?;
        lease.call(arg)
    };
    flush_if_idle(state);
    result
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

pub(crate) fn node_ref_get<'scope, T: Clone>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<Option<T>> {
    with_node_ref(state, id, |value| {
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
    update_node_ref(state, id, |stored| {
        unsafe { stored.downcast_mut::<Option<T>>() }
            .map(|slot| *slot = Some(value))
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

pub(crate) fn node_ref_clear<'scope, T>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<()> {
    update_node_ref(state, id, |stored| {
        unsafe { stored.downcast_mut::<Option<T>>() }
            .map(|slot| *slot = None)
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

pub(crate) fn notify<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<()> {
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

pub(crate) fn with_untracked<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    f: impl FnOnce() -> R,
) -> R {
    let scheduler = state.borrow().scheduler.clone();
    let frame = super::scheduler::ObserverFrame::push_untracked(scheduler);
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
        Scope,
        internal::value::AnyValue,
        runtime::{
            dispose_nodes,
            model::NodeState,
            scheduler::{GlobalScheduler, ScheduledTask},
        },
        scope::ScopeStorage,
    };
    use std::{marker::PhantomData, rc::Rc};

    #[test]
    fn disposing_a_node_during_a_read_does_not_require_put_back() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let state = storage.owner_token(PhantomData).state();
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

    #[test]
    fn final_cleanup_stored_access_does_not_flush_another_scope() {
        let scheduler = GlobalScheduler::new();
        let active_storage = ScopeStorage::new(scheduler.clone());
        let disposing_storage = ScopeStorage::new(scheduler.clone());
        let active_scope = Scope {
            storage: &active_storage,
            _marker: PhantomData,
        };
        let disposing_scope = Scope {
            storage: &disposing_storage,
            _marker: PhantomData,
        };
        let (source, _) = active_scope
            .signal(0_i32)
            .expect("fallible reactive creation");
        let runs = Rc::new(std::cell::Cell::new(0));
        let runs_in_effect = runs.clone();
        let effect = active_scope
            .effect(
                move || {
                    let _ = source.get();
                    runs_in_effect.set(runs_in_effect.get() + 1);
                    Ok(())
                },
                active_scope
                    .error_handler(|_: ()| {})
                    .expect("handler registration"),
            )
            .expect("effect should initialize");
        assert_eq!(runs.get(), 1);

        {
            let state = effect.handle.state();
            let mut state_ref = state.borrow_mut();
            let scope_id = state_ref.scope_id;
            let node = state_ref
                .nodes
                .get_mut(effect.handle.raw())
                .expect("effect node should exist");
            node.state = NodeState::Dirty;
            node.queued = true;
            drop(state_ref);
            scheduler.borrow_mut().enqueue_effect(ScheduledTask {
                scope_id,
                node: effect.handle.raw(),
            });
        }

        let stored = disposing_scope
            .stored(1_i32)
            .expect("fallible reactive creation");
        let runs_in_cleanup = runs.clone();
        disposing_scope
            .on_cleanup(
                move || {
                    assert_eq!(runs_in_cleanup.get(), 1);
                    stored
                        .update(|value| *value = 2)
                        .expect("stored value should be writable during final cleanup");
                    assert_eq!(runs_in_cleanup.get(), 1);
                    Ok(())
                },
                disposing_scope
                    .error_handler(|_: ()| {})
                    .expect("handler registration"),
            )
            .expect("cleanup should register");

        disposing_storage.dispose_untracked();
        assert_eq!(runs.get(), 2);

        active_storage.dispose_untracked();
    }

    #[test]
    fn disposed_stored_value_cannot_access_a_reused_scope_id() {
        let scheduler = GlobalScheduler::new();
        let first_storage = ScopeStorage::new(scheduler.clone());
        let first_scope = Scope {
            storage: &first_storage,
            _marker: PhantomData,
        };
        let stored = first_scope
            .stored(1_i32)
            .expect("fallible reactive creation");
        first_storage.dispose_untracked();

        let replacement = ScopeStorage::new(scheduler);
        assert_eq!(stored.with(|value| *value), Err(ReactiveError::NoSuchNode));
        replacement.dispose_untracked();
    }
}
