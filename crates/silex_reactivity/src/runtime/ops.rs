//! Operations on reactivity nodes (Signal, Stored, Callback, NodeRef) and runtime execution scoping.

use super::{
    eval::{flush_if_idle, prepare_read},
    model::{Payload, ScopeState},
    scheduler::GlobalScheduler,
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

struct ValueBorrow {
    scheduler: Rc<RefCell<GlobalScheduler>>,
}

impl ValueBorrow {
    fn new(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        scheduler.borrow_mut().borrowed_values += 1;
        Self { scheduler }
    }
}

impl Drop for ValueBorrow {
    fn drop(&mut self) {
        let mut scheduler = self.scheduler.borrow_mut();
        scheduler.borrowed_values = scheduler.borrowed_values.saturating_sub(1);
    }
}

fn with_taken_value<T, R>(
    take: impl FnOnce() -> ReactiveResult<(T, ValueBorrow)>,
    restore: impl FnOnce(T, bool) -> ReactiveResult<()>,
    exec: impl FnOnce(&mut T) -> R,
) -> ReactiveResult<R> {
    let (mut value, borrow) = take()?;
    let result = catch_unwind(AssertUnwindSafe(|| exec(&mut value)));
    let put_back = restore(value, false);
    drop(borrow);
    if let Err(panic) = result {
        let _ = put_back;
        resume_unwind(panic);
    }
    put_back?;
    result.map_err(|_| ReactiveError::Reentrant)
}

fn update_taken_value<T, R>(
    take: impl FnOnce() -> ReactiveResult<(T, ValueBorrow)>,
    restore: impl FnOnce(T, bool) -> ReactiveResult<()>,
    exec: impl FnOnce(&mut T) -> (R, bool),
) -> ReactiveResult<(R, bool)> {
    let (value, borrow) = take()?;
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
            drop(borrow);
            resume_unwind(panic);
        }
    };
    let value = value.take().expect("value is taken out");
    restore(value, changed)?;
    drop(borrow);
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
            let value = state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::Reentrant)?
                .take_value(id, NodeKindTag::Signal)?;
            let scheduler = state
                .try_borrow()
                .map_err(|_| ReactiveError::Reentrant)?
                .scheduler
                .clone();
            Ok((value, ValueBorrow::new(scheduler)))
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
            let value = state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::Reentrant)?
                .take_value(id, NodeKindTag::Signal)?;
            let scheduler = state
                .try_borrow()
                .map_err(|_| ReactiveError::Reentrant)?
                .scheduler
                .clone();
            Ok((value, ValueBorrow::new(scheduler)))
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
    let result = with_taken_value(
        || take_stored(state, id),
        |value, _| put_stored(state, id, value),
        |val| f(val),
    )?;
    flush_if_idle(state);
    Ok(result)
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
    flush_if_idle(state);
    Ok(result)
}

fn take_stored<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> ReactiveResult<(AnyValue, ValueBorrow)> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    let node = state_ref.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
    if !matches!(node.kind, NodeKindTag::Stored | NodeKindTag::NodeRef) {
        return Err(ReactiveError::WrongKind);
    }
    let data = state_ref
        .data
        .get_mut(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    match data.payload.take() {
        Some(Payload::Stored(value)) => {
            let scheduler = state_ref.scheduler.clone();
            Ok((value, ValueBorrow::new(scheduler)))
        }
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
    let _node = state_ref.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
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
) -> ReactiveResult<(CallbackThunk<'scope>, ValueBorrow)> {
    let mut state_ref = state
        .try_borrow_mut()
        .map_err(|_| ReactiveError::Reentrant)?;
    let node = state_ref.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
    if node.kind != NodeKindTag::Callback {
        return Err(ReactiveError::WrongKind);
    }
    let data = state_ref
        .data
        .get_mut(id)
        .ok_or(ReactiveError::NoSuchNode)?;
    match data.payload.take() {
        Some(Payload::Callback(callback)) => {
            let scheduler = state_ref.scheduler.clone();
            Ok((callback, ValueBorrow::new(scheduler)))
        }
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
    )?;
    flush_if_idle(state);
    Ok(())
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
    let should_flush = {
        let mut state_ref = state
            .try_borrow_mut()
            .expect("ScopeState borrow failed during notify");
        if state_ref.mark_notified(id) {
            state_ref.queue_dependents(id);
        }
        let source_available = state_ref
            .data
            .get(id)
            .is_some_and(|data| data.value.is_some());
        source_available && state_ref.scheduler.borrow().should_flush()
    };
    if should_flush {
        flush_if_idle(state);
    }
}

pub(crate) fn track<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) {
    let mut state_ref = state
        .try_borrow_mut()
        .expect("ScopeState borrow failed during track");
    state_ref.track(id);
}

pub(crate) fn track_many<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, ids: &[RawId]) {
    let mut state_ref = state
        .try_borrow_mut()
        .expect("ScopeState borrow failed during track_many");
    state_ref.track_many(ids);
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
