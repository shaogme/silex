//! Opaque scheduler-family inputs and the computation creation kernel.

use super::eval::flush_if_idle;
use super::{
    eval::EvaluationError,
    model::ScopeState,
    scheduler::{GlobalScheduler, InitialFlushGuard},
};
use crate::{
    EffectInitError, EffectInitResult, ErrorHandler, ReactiveError, ReactiveResult,
    child::WatchOptions,
    error::{ErrorPhase, InitialErrorSlot},
    internal::{
        RawId,
        value::{Computation, DerivedThunk, EffectThunk, MemoThunk, PreviousThunk, WatchThunk},
    },
};
use smallvec::SmallVec;
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

const INLINE_INPUTS: usize = 4;

/// Opaque scheduler-family provenance for a reactive source.
///
/// The value has no public constructor, identity accessor, or node operation.
/// It only gives a computation creation boundary enough information to reject
/// a source from another scheduler family.
#[doc(hidden)]
#[derive(Clone)]
pub struct RuntimeInput {
    scheduler: Rc<RefCell<GlobalScheduler>>,
}

impl std::fmt::Debug for RuntimeInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RuntimeInput(..)")
    }
}

impl PartialEq for RuntimeInput {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.scheduler, &other.scheduler)
    }
}

impl Eq for RuntimeInput {}

impl RuntimeInput {
    pub(crate) fn from_scheduler(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        Self { scheduler }
    }

    #[inline]
    fn belongs_to(&self, scheduler: &Rc<RefCell<GlobalScheduler>>) -> bool {
        Rc::ptr_eq(&self.scheduler, scheduler)
    }
}

/// Opaque inline-first collection of source provenance claims.
#[doc(hidden)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeInputs {
    inputs: SmallVec<[RuntimeInput; INLINE_INPUTS]>,
}

impl RuntimeInputs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn single(input: RuntimeInput) -> Self {
        let mut inputs = Self::new();
        inputs.push(input);
        inputs
    }

    pub fn push(&mut self, input: RuntimeInput) {
        self.inputs.push(input);
    }

    pub fn extend(&mut self, other: &Self) {
        self.inputs.extend(other.inputs.iter().cloned());
    }

    pub fn len(&self) -> usize {
        self.inputs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &RuntimeInput> {
        self.inputs.iter()
    }
}

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
    pub(crate) inputs: RuntimeInputs,
    pub(crate) computation: Computation<'scope>,
}

/// Validate target liveness and all source-family claims without mutating the
/// target state, graph, scheduler, or queue.
pub(crate) fn validate_inputs<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    inputs: &RuntimeInputs,
) -> ReactiveResult<()> {
    let (scheduler, scope_id, active) = {
        let state = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        (state.scheduler.clone(), state.scope_id, state.is_active())
    };
    let scheduler_ref = scheduler
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?;

    if !active || !scheduler_ref.is_scope_active(scope_id) {
        return Err(ReactiveError::NoSuchNode);
    }
    if inputs.iter().any(|input| !input.belongs_to(&scheduler)) {
        return Err(ReactiveError::RuntimeMismatch);
    }
    Ok(())
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
    validate_inputs(state, &spec.inputs).map_err(EvaluationError::Runtime)?;

    let scheduler = state
        .try_borrow()
        .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
        .scheduler
        .clone();
    let initial_flush_guard =
        InitialFlushGuard::try_new(scheduler).map_err(EvaluationError::Runtime)?;

    let ComputationSpec {
        kind,
        inputs: _,
        computation,
    } = spec;
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
) -> EffectInitResult<RawId, E> {
    match result {
        Ok(raw) => Ok(raw),
        Err(EvaluationError::Runtime(error)) => Err(EffectInitError::Registration(error)),
        Err(EvaluationError::Callback(error)) => {
            error.dispatch(ErrorPhase::Initial);
            let initial = initial_slot.take();
            flush_if_idle(state);
            Err(EffectInitError::Initial(initial))
        }
        Err(EvaluationError::User(_)) => {
            Err(EffectInitError::Registration(ReactiveError::TypeMismatch))
        }
    }
}

fn finish_infallible_creation(result: Result<RawId, EvaluationError<'_>>) -> ReactiveResult<RawId> {
    match result {
        Ok(raw) => Ok(raw),
        Err(EvaluationError::Runtime(error)) => Err(error),
        Err(EvaluationError::Callback(_)) => {
            unreachable!("infallible computations cannot return callback errors")
        }
        Err(EvaluationError::User(_)) => {
            unreachable!("infallible computations cannot return user errors")
        }
    }
}

pub(crate) fn create_effect<'scope, E, F>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    inputs: RuntimeInputs,
    callback: F,
    handler: ErrorHandler<'scope, E>,
) -> EffectInitResult<RawId, E>
where
    E: 'scope,
    F: FnMut() -> Result<(), E> + 'scope,
{
    let initial_slot = InitialErrorSlot::new();
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Effect,
            inputs,
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
    inputs: RuntimeInputs,
    callback: F,
    handler: ErrorHandler<'scope, E>,
) -> EffectInitResult<RawId, E>
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
            inputs,
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
    inputs: RuntimeInputs,
    getter: G,
    callback: C,
    handler: ErrorHandler<'scope, E>,
    options: WatchOptions,
) -> EffectInitResult<RawId, E>
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
            inputs,
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

pub(crate) fn create_memo<'scope, T, F>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    inputs: RuntimeInputs,
    callback: F,
) -> ReactiveResult<RawId>
where
    T: PartialEq + 'scope,
    F: FnMut(Option<&T>) -> T + 'scope,
{
    finish_infallible_creation(create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Memo,
            inputs,
            computation: Computation::Memo(MemoThunk::new::<T, F>(callback)),
        },
    ))
}

pub(crate) fn create_derived<'scope, T, E, F>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    inputs: RuntimeInputs,
    callback: F,
    handler: ErrorHandler<'scope, E>,
) -> EffectInitResult<RawId, E>
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
            inputs,
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
    use crate::{Scope, internal::value::AnyValue, scope::ScopeStorage};
    use std::{cell::RefCell, marker::PhantomData, rc::Rc};

    fn snapshot(storage: &ScopeStorage) -> (usize, usize, usize, u64, bool, bool) {
        let state = storage.state.borrow();
        let cleanup_count = state.root_cleanups.len()
            + state
                .data
                .values()
                .map(|data| data.cleanups.len())
                .sum::<usize>();
        let scheduler = state.scheduler.borrow();
        (
            state.nodes.len(),
            cleanup_count,
            scheduler.global_queue.len(),
            scheduler.current_epoch(),
            scheduler.observer().is_some(),
            scheduler.running_queue,
        )
    }

    fn handler<'scope>(storage: &'scope ScopeStorage) -> ErrorHandler<'scope, ()> {
        let scope = Scope {
            storage,
            _marker: PhantomData,
        };
        scope.error_handler(|_| {}).expect("handler registration")
    }

    #[test]
    fn mismatched_input_is_rejected_before_any_state_changes() {
        let source = ScopeStorage::new(GlobalScheduler::new());
        let target = ScopeStorage::new(GlobalScheduler::new());
        let source_state = unsafe { source.typed_state() };
        source_state
            .borrow_mut()
            .create_signal(AnyValue::new(1_i32))
            .expect("test scope should be active");
        let input = RuntimeInput::from_scheduler(source.scheduler());
        let target_state = unsafe { target.typed_state() };
        let before = snapshot(&target);

        let result = create_effect(
            &target_state,
            RuntimeInputs::single(input),
            || Ok(()),
            handler(&target),
        );

        assert!(matches!(
            result,
            Err(EffectInitError::Registration(
                ReactiveError::RuntimeMismatch
            ))
        ));
        assert_eq!(snapshot(&target), before);

        target.dispose();
        source.dispose();
    }

    #[test]
    fn inactive_target_is_reported_before_input_compatibility() {
        let source = ScopeStorage::new(GlobalScheduler::new());
        let target = ScopeStorage::new(GlobalScheduler::new());
        let input = RuntimeInput::from_scheduler(source.scheduler());
        let target_state = unsafe { target.typed_state() };
        let error_handler = handler(&target);
        target.dispose();

        let result = create_effect(
            &target_state,
            RuntimeInputs::single(input),
            || Ok(()),
            error_handler,
        );

        assert!(matches!(
            result,
            Err(EffectInitError::Registration(ReactiveError::NoSuchNode))
        ));
        source.dispose();
    }

    #[test]
    fn every_input_is_validated_before_registration() {
        let scheduler = GlobalScheduler::new();
        let same_family = ScopeStorage::new(scheduler);
        let foreign = ScopeStorage::new(GlobalScheduler::new());
        let target_state = unsafe { same_family.typed_state() };
        let same_input = RuntimeInput::from_scheduler(same_family.scheduler());
        let foreign_input = RuntimeInput::from_scheduler(foreign.scheduler());
        let before = snapshot(&same_family);
        let ran = Rc::new(RefCell::new(false));
        let ran_in_callback = ran.clone();
        let mut inputs = RuntimeInputs::single(same_input);
        inputs.push(foreign_input);

        let result = create_effect(
            &target_state,
            inputs,
            move || {
                *ran_in_callback.borrow_mut() = true;
                Ok(())
            },
            handler(&same_family),
        );

        assert!(matches!(
            result,
            Err(EffectInitError::Registration(
                ReactiveError::RuntimeMismatch
            ))
        ));
        assert!(!*ran.borrow());
        assert_eq!(snapshot(&same_family), before);

        same_family.dispose();
        foreign.dispose();
    }

    #[test]
    fn same_scheduler_input_reaches_initial_run_after_registration() {
        let scheduler = GlobalScheduler::new();
        let source = ScopeStorage::new(scheduler.clone());
        let target = ScopeStorage::new(scheduler);
        let input = RuntimeInput::from_scheduler(source.scheduler());
        let target_state = unsafe { target.typed_state() };
        let ran = Rc::new(RefCell::new(false));
        let ran_in_callback = ran.clone();

        let result = create_effect(
            &target_state,
            RuntimeInputs::single(input),
            move || {
                *ran_in_callback.borrow_mut() = true;
                Ok(())
            },
            handler(&target),
        );

        assert!(result.is_ok());
        assert!(*ran.borrow());
        assert_eq!(target_state.borrow().nodes.len(), 1);

        target.dispose();
        source.dispose();
    }
}
