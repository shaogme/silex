//! Operations on reactivity nodes (Signal, Stored, Callback, NodeRef) and runtime execution scoping.

use super::{
    eval::{flush_if_idle, prepare_read},
    model::{Payload, ScopeState},
};
use crate::{
    ReactiveError, ReactiveResult,
    handle::NodeKindTag,
    internal::{
        RawId,
        value::{AnyValue, CallbackThunk},
    },
};
use std::{
    any::Any,
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

fn with_taken_value<T, R>(
    take: impl FnOnce() -> ReactiveResult<T>,
    restore: impl FnOnce(T, bool) -> ReactiveResult<()>,
    exec: impl FnOnce(&mut T) -> R,
) -> ReactiveResult<R> {
    let mut value = take()?;
    let result = catch_unwind(AssertUnwindSafe(|| exec(&mut value)));
    let put_back = restore(value, false);
    if let Err(panic) = result {
        let _ = put_back;
        resume_unwind(panic);
    }
    put_back?;
    result.map_err(|_| ReactiveError::Reentrant)
}

fn update_taken_value<T, R>(
    take: impl FnOnce() -> ReactiveResult<T>,
    restore: impl FnOnce(T, bool) -> ReactiveResult<()>,
    exec: impl FnOnce(&mut T) -> (R, bool),
) -> ReactiveResult<(R, bool)> {
    let value = take()?;
    let mut value = Some(value);
    let result = catch_unwind(AssertUnwindSafe(|| {
        exec(value.as_mut().expect("value is taken out"))
    }));
    let ((result, changed), _panicked) = match result {
        Ok(val) => (val, false),
        Err(panic) => {
            if let Some(val) = value.take() {
                let _ = restore(val, false);
            }
            resume_unwind(panic);
        }
    };
    let value = value.take().expect("value is taken out");
    restore(value, changed)?;
    Ok((result, changed))
}

pub(crate) fn with_signal<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    track: bool,
    f: impl FnOnce(&AnyValue) -> R,
) -> ReactiveResult<R> {
    prepare_read(state, id, track)?;
    let result = with_taken_value(
        || {
            state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::Reentrant)?
                .take_value(id, NodeKindTag::Signal)
        },
        |value, bump| {
            state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::Reentrant)
                .map(|mut state_ref| {
                    state_ref.put_value(id, value, bump);
                })
        },
        |val| f(val),
    )?;
    flush_if_idle(state);
    Ok(result)
}

pub(crate) fn update_signal<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&mut AnyValue) -> (R, bool),
) -> ReactiveResult<R> {
    let (result, _) = update_taken_value(
        || {
            state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::Reentrant)?
                .take_value(id, NodeKindTag::Signal)
        },
        |value, bump| {
            state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::Reentrant)
                .map(|mut state_ref| {
                    state_ref.put_value(id, value, bump);
                    if bump {
                        state_ref.queue_dependents(id);
                    }
                })
        },
        f,
    )?;
    flush_if_idle(state);
    Ok(result)
}

pub(crate) fn with_stored<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&AnyValue) -> R,
) -> ReactiveResult<R> {
    with_taken_value(
        || take_stored(state, id),
        |value, _| put_stored(state, id, value),
        |val| f(val),
    )
}

pub(crate) fn update_stored<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    f: impl FnOnce(&mut AnyValue) -> R,
) -> ReactiveResult<R> {
    let (result, _) = update_taken_value(
        || take_stored(state, id),
        |value, _| put_stored(state, id, value),
        |val| (f(val), false),
    )?;
    Ok(result)
}

fn take_stored<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<AnyValue> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    let node = state_ref
        .nodes
        .get(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    if !matches!(node.kind, NodeKindTag::Stored | NodeKindTag::NodeRef) {
        return Err(ReactiveError::WrongKind);
    }
    let data = state_ref
        .data
        .get_mut(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    match data.payload.take() {
        Some(Payload::Stored(value)) => Ok(value),
        Some(p) => {
            data.payload = Some(p);
            Err(ReactiveError::WrongKind)
        }
        None => Err(ReactiveError::Reentrant),
    }
}

fn put_stored<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    value: AnyValue,
) -> ReactiveResult<()> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    let _node = state_ref
        .nodes
        .get(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    let data = state_ref
        .data
        .get_mut(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    data.payload = Some(Payload::Stored(value));
    Ok(())
}

fn take_callback<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<CallbackThunk<'scope>> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    let node = state_ref
        .nodes
        .get(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    if node.kind != NodeKindTag::Callback {
        return Err(ReactiveError::WrongKind);
    }
    let data = state_ref
        .data
        .get_mut(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    match data.payload.take() {
        Some(Payload::Callback(callback)) => Ok(callback),
        Some(p) => {
            data.payload = Some(p);
            Err(ReactiveError::WrongKind)
        }
        None => Err(ReactiveError::Reentrant),
    }
}

fn put_callback<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    callback: CallbackThunk<'scope>,
) -> ReactiveResult<()> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    if let Some(data) = state_ref.data.get_mut(id) {
        data.payload = Some(Payload::Callback(callback));
    }
    Ok(())
}

pub(crate) fn invoke_callback<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    arg: Box<dyn Any>,
) -> ReactiveResult<()> {
    with_taken_value(
        || take_callback(state, id),
        |cb, _| put_callback(state, id, cb),
        |cb| cb.call(arg),
    )
}

pub(crate) fn node_ref_get<'scope, T: Clone + 'static>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<Option<T>> {
    with_stored(state, id, |value| {
        value
            .downcast_ref::<Option<T>>()
            .cloned()
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

pub(crate) fn node_ref_set<'scope, T: 'static>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    value: T,
) -> ReactiveResult<()> {
    update_stored(state, id, |stored| {
        stored
            .downcast_mut::<Option<T>>()
            .map(|slot| *slot = Some(value))
            .ok_or(ReactiveError::TypeMismatch)
    })?
}

pub(crate) fn notify<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) {
    if let Ok(mut state_ref) = state.try_borrow_mut() {
        state_ref.queue_dependents(id);
    }
    flush_if_idle(state);
}

pub(crate) fn track<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) {
    if let Ok(mut state_ref) = state.try_borrow_mut() {
        state_ref.track(id);
    }
}

pub(crate) fn track_many<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, ids: &[RawId]) {
    if let Ok(mut state_ref) = state.try_borrow_mut() {
        state_ref.track_many(ids);
    }
}

pub(crate) fn with_untracked<'scope, R>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    f: impl FnOnce() -> R,
) -> R {
    let scheduler = state.borrow().scheduler.clone();
    let previous = scheduler.borrow_mut().set_observer(None);
    let result = catch_unwind(AssertUnwindSafe(f));
    scheduler.borrow_mut().set_observer(previous);
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
