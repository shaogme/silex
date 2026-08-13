//! Computation creation kernel.

use super::eval::flush_if_idle;
use super::{eval::EvaluationError, model::ScopeState, scheduler::InitialFlushGuard};
use crate::{
    ComputationInitError, ComputationInitResult, ErrorHandler, ReactiveError,
    child::WatchOptions,
    error::{ErrorPhase, InitialErrorSlot},
    internal::{
        RawId,
        value::{Computation, DerivedThunk, EffectThunk, MemoThunk, PreviousThunk, WatchThunk},
    },
};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

#[derive(Clone, Copy)]
pub(crate) enum ComputationKind {
    Effect,
    Previous,
    Watch,
    Memo,
    Derived,
}

pub(crate) struct ComputationSpec<'scope> {
    pub(crate) kind: ComputationKind,
    pub(crate) computation: Computation<'scope>,
}

/// Create one computation through the single validation and registration
/// boundary.
///
/// The order is deliberately fixed: target validation, input validation,
/// node registration, and only then the initial run.
pub(crate) fn create_computation<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    spec: ComputationSpec<'scope>,
) -> Result<RawId, EvaluationError<'scope>> {
    let active = state
        .try_borrow()
        .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
        .is_active();
    if !active {
        return Err(EvaluationError::Runtime(ReactiveError::NoSuchNode));
    }

    let scheduler = state
        .try_borrow()
        .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
        .scheduler
        .clone();
    let initial_flush_guard =
        InitialFlushGuard::try_new(scheduler).map_err(EvaluationError::Runtime)?;

    let ComputationSpec { kind, computation } = spec;
    let raw = {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
        match (kind, computation) {
            (ComputationKind::Effect, Computation::Effect(callback)) => state
                .register_effect(callback)
                .map_err(EvaluationError::Runtime)?,
            (ComputationKind::Previous, Computation::Previous(callback)) => state
                .register_previous(callback)
                .map_err(EvaluationError::Runtime)?,
            (ComputationKind::Watch, Computation::Watch(callback)) => state
                .register_watch(callback)
                .map_err(EvaluationError::Runtime)?,
            (ComputationKind::Memo, Computation::Memo(callback)) => state
                .register_memo(callback, false)
                .map_err(EvaluationError::Runtime)?,
            (ComputationKind::Derived, Computation::Derived(callback)) => state
                .register_derived(callback)
                .map_err(EvaluationError::Runtime)?,
            _ => return Err(EvaluationError::Runtime(ReactiveError::WrongKind)),
        }
    };

    let result = catch_unwind(AssertUnwindSafe(|| super::run_initial(state, raw)));
    match result {
        Ok(Ok(())) => {
            drop(initial_flush_guard);
            flush_if_idle(state);
            Ok(raw)
        }
        Ok(Err(error)) => {
            let dispose = catch_unwind(AssertUnwindSafe(|| super::dispose_nodes(state, vec![raw])));
            if let Err(panic) = dispose {
                drop(initial_flush_guard);
                resume_unwind(panic);
            }
            drop(initial_flush_guard);
            Err(error)
        }
        Err(panic) => {
            let dispose = catch_unwind(AssertUnwindSafe(|| super::dispose_nodes(state, vec![raw])));
            if let Err(dispose_panic) = dispose {
                drop(initial_flush_guard);
                resume_unwind(dispose_panic);
            }
            drop(initial_flush_guard);
            resume_unwind(panic);
        }
    }
}

fn finish_creation<'scope, E>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    result: Result<RawId, EvaluationError<'scope>>,
    initial_slot: &InitialErrorSlot<E>,
) -> ComputationInitResult<RawId, E> {
    match result {
        Ok(raw) => Ok(raw),
        Err(EvaluationError::Runtime(error)) => Err(ComputationInitError::Registration(error)),
        Err(EvaluationError::Callback(error)) => {
            error.dispatch(ErrorPhase::Initial);
            let initial = initial_slot.take();
            flush_if_idle(state);
            Err(ComputationInitError::Initial(initial))
        }
        Err(EvaluationError::User(_)) => Err(ComputationInitError::Registration(
            ReactiveError::TypeMismatch,
        )),
    }
}

pub(crate) fn create_effect<'scope, E, F>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
    handler: ErrorHandler<'scope, E>,
) -> ComputationInitResult<RawId, E>
where
    E: 'scope,
    F: FnMut() -> Result<(), E> + 'scope,
{
    let initial_slot = InitialErrorSlot::new();
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Effect,
            computation: Computation::Effect(EffectThunk::new(
                callback,
                handler,
                initial_slot.clone(),
            )),
        },
    );
    finish_creation(state, result, &initial_slot)
}

pub(crate) fn create_previous<'scope, T, E, F>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
    handler: ErrorHandler<'scope, E>,
) -> ComputationInitResult<RawId, E>
where
    T: 'scope,
    E: 'scope,
    F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
{
    let initial_slot = InitialErrorSlot::new();
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Previous,
            computation: Computation::Previous(PreviousThunk::new::<T, E, F>(
                callback,
                handler,
                initial_slot.clone(),
            )),
        },
    );
    finish_creation(state, result, &initial_slot)
}

pub(crate) fn create_watch<'scope, T, E, G, C>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    getter: G,
    callback: C,
    handler: ErrorHandler<'scope, E>,
    options: WatchOptions,
) -> ComputationInitResult<RawId, E>
where
    T: PartialEq + 'scope,
    E: 'scope,
    G: FnMut() -> Result<T, E> + 'scope,
    C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
{
    let initial_slot = InitialErrorSlot::new();
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Watch,
            computation: Computation::Watch(WatchThunk::new::<T, E, G, C>(
                getter,
                callback,
                handler,
                initial_slot.clone(),
                options.immediate,
                options.once,
            )),
        },
    );
    finish_creation(state, result, &initial_slot)
}

pub(crate) fn create_memo<'scope, T, E, F>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
    handler: ErrorHandler<'scope, E>,
) -> ComputationInitResult<RawId, E>
where
    T: PartialEq + 'scope,
    E: 'scope,
    F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
{
    let initial_slot = InitialErrorSlot::new();
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Memo,
            computation: Computation::Memo(MemoThunk::new::<T, E, F>(
                callback,
                handler,
                initial_slot.clone(),
            )),
        },
    );
    finish_creation(state, result, &initial_slot)
}

pub(crate) fn create_derived<'scope, T, E, F>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    callback: F,
    handler: ErrorHandler<'scope, E>,
) -> ComputationInitResult<RawId, E>
where
    T: 'scope,
    E: 'scope,
    F: FnMut() -> Result<T, E> + 'scope,
{
    let initial_slot = InitialErrorSlot::new();
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Derived,
            computation: Computation::Derived(DerivedThunk::new(
                callback,
                handler,
                initial_slot.clone(),
            )),
        },
    );
    finish_creation(state, result, &initial_slot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Scope, runtime::GlobalScheduler, scope::ScopeStorage};
    use std::{cell::Cell, marker::PhantomData, rc::Rc};

    fn scope<'scope>(storage: &'scope ScopeStorage) -> Scope<'scope> {
        Scope {
            storage,
            _marker: PhantomData,
        }
    }

    #[test]
    fn initial_effect_runs_only_after_registration() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = scope(&storage);
        let runs = Rc::new(Cell::new(0));
        let runs_in_callback = runs.clone();
        let handler = scope.error_handler(|_| {}).expect("handler registration");

        let result = create_effect(
            &unsafe { storage.typed_state() },
            move || {
                runs_in_callback.set(runs_in_callback.get() + 1);
                Ok::<(), ()>(())
            },
            handler,
        );

        assert!(result.is_ok());
        assert_eq!(runs.get(), 1);
        storage.dispose();
    }

    #[test]
    fn inactive_target_is_rejected_without_running_the_callback() {
        let storage = ScopeStorage::new(GlobalScheduler::new());
        let scope = scope(&storage);
        let handler = scope.error_handler(|_| {}).expect("handler registration");
        storage.dispose();
        let result = create_effect(
            &unsafe { storage.typed_state() },
            || Ok::<(), ()>(()),
            handler,
        );

        assert!(matches!(
            result,
            Err(ComputationInitError::Registration(
                ReactiveError::NoSuchNode
            ))
        ));
    }
}
