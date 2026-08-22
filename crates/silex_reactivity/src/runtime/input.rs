//! Computation creation kernel.

use super::{
    dispose_nodes,
    eval::{self, EvaluationError, flush_if_idle},
    model::{ComputationParent, ScopeState},
    scheduler::{InitialFlushGuard, ObserverFrame},
    storage::{
        ChangePredicate, ComputationBehavior, ComputationExecutionError, ComputedEvaluation,
        ComputedEvaluator, ComputedNode, TypedNodeRef,
    },
};
use crate::{
    ComputationInitError, ComputationInitResult, ErrorHandlerRef, ReactiveError,
    error::{ErrorEvent, ErrorPhase, ErrorSlotOwner, ErrorSlotRef},
    handle::NodeKindTag,
    internal::NodeId,
    owner::{EffectPhase, ScopeStorage, WatchOptions},
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Clone, Copy)]
pub(crate) enum ComputationKind {
    Effect,
    Previous,
    Watch,
    Computed,
}

impl ComputationKind {
    fn tag(self) -> NodeKindTag {
        match self {
            Self::Effect | Self::Previous | Self::Watch => NodeKindTag::Effect,
            Self::Computed => NodeKindTag::Computed,
        }
    }
}

pub(crate) struct ComputationSpec<'scope> {
    pub(crate) kind: ComputationKind,
    pub(crate) phase: EffectPhase,
    pub(crate) parent: ComputationParent,
    pub(crate) computation: Box<dyn ComputationBehavior<'scope> + 'scope>,
}

pub(crate) struct TypedComputation<'scope, T, E> {
    pub(crate) raw: NodeId,
    pub(crate) value: TypedNodeRef<'scope, T>,
    pub(crate) errors: ErrorSlotRef<'scope, E>,
}

pub(crate) fn create_computation<'scope>(
    state: &ScopeState<'scope>,
    spec: ComputationSpec<'scope>,
) -> Result<NodeId, EvaluationError<'scope>> {
    let active = state
        .try_borrow()
        .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
        .is_active()
        .map_err(EvaluationError::Runtime)?;
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

    let raw = {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
        state
            .register_computation(spec.kind.tag(), spec.phase, spec.computation, spec.parent)
            .map_err(EvaluationError::Runtime)?
    };

    let result = catch_unwind(AssertUnwindSafe(|| eval::run_initial(state, raw)));
    match result {
        Ok(Ok(())) => {
            drop(initial_flush_guard);
            flush_if_idle(state).map_err(EvaluationError::Runtime)?;
            Ok(raw)
        }
        Ok(Err(error)) => {
            let dispose = catch_unwind(AssertUnwindSafe(|| dispose_nodes(state, vec![raw])));
            match dispose {
                Err(panic) => {
                    drop(initial_flush_guard);
                    resume_unwind(panic);
                }
                Ok(Err(dispose_error)) => {
                    drop(initial_flush_guard);
                    return Err(EvaluationError::Runtime(dispose_error));
                }
                Ok(Ok(_)) => {}
            }
            drop(initial_flush_guard);
            Err(error)
        }
        Err(panic) => {
            let dispose = catch_unwind(AssertUnwindSafe(|| dispose_nodes(state, vec![raw])));
            match dispose {
                Err(dispose_panic) => {
                    drop(initial_flush_guard);
                    resume_unwind(dispose_panic);
                }
                Ok(Err(dispose_error)) => {
                    drop(initial_flush_guard);
                    return Err(EvaluationError::Runtime(dispose_error));
                }
                Ok(Ok(_)) => {}
            }
            drop(initial_flush_guard);
            resume_unwind(panic);
        }
    }
}

fn finish_creation<'scope, E>(
    state: &ScopeState<'scope>,
    result: Result<NodeId, EvaluationError<'scope>>,
    errors: ErrorSlotOwner<'scope, E>,
) -> ComputationInitResult<NodeId, E> {
    match result {
        Ok(raw) => Ok(raw),
        Err(EvaluationError::Runtime(error)) => Err(ComputationInitError::Registration(error)),
        Err(EvaluationError::Callback(error)) => {
            error.dispatch(ErrorPhase::Initial).map_err(|error| {
                ComputationInitError::Registration(ReactiveError::Handler(error))
            })?;
            let initial = errors.take().map_err(ComputationInitError::Registration)?;
            flush_if_idle(state).map_err(ComputationInitError::Registration)?;
            Err(ComputationInitError::Initial(initial))
        }
        Err(EvaluationError::Handler(error)) => Err(ComputationInitError::Registration(
            ReactiveError::Handler(error),
        )),
        Err(EvaluationError::User) => Err(ComputationInitError::Registration(
            ReactiveError::InvariantViolation,
        )),
    }
}

pub(crate) fn create_effect<'scope, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    phase: EffectPhase,
    callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<NodeId, E>
where
    E: 'scope,
    F: FnMut() -> Result<(), E> + 'scope,
{
    create_effect_with_parent(
        storage,
        state,
        phase,
        callback,
        handler,
        ComputationParent::Current,
    )
}

pub(crate) fn create_effect_detached<'scope, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    phase: EffectPhase,
    callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<NodeId, E>
where
    E: 'scope,
    F: FnMut() -> Result<(), E> + 'scope,
{
    create_effect_with_parent(
        storage,
        state,
        phase,
        callback,
        handler,
        ComputationParent::Detached,
    )
}

fn create_effect_with_parent<'scope, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    phase: EffectPhase,
    mut callback: F,
    handler: ErrorHandlerRef<'scope, E>,
    parent: ComputationParent,
) -> ComputationInitResult<NodeId, E>
where
    E: 'scope,
    F: FnMut() -> Result<(), E> + 'scope,
{
    let handler = match handler.lease() {
        Ok(handler) => handler,
        Err(error) => {
            return Err(ComputationInitError::Registration(ReactiveError::Handler(
                error,
            )));
        }
    };
    let errors = storage.alloc_error_slot();
    let evaluator_errors = errors.clone();
    let evaluator: ComputedEvaluator<'scope, ()> = Box::new(move |_previous, _scheduler| {
        callback()
            .map(|()| ComputedEvaluation {
                value: (),
                stop_after_run: false,
            })
            .map_err(|error| {
                ComputationExecutionError::Callback(ErrorEvent::new(
                    error,
                    &handler,
                    evaluator_errors.clone(),
                ))
            })
    });
    let computation = ComputedNode::<_, E>::new(
        None,
        errors.reference(),
        evaluator,
        Box::new(|_, _| true),
        false,
    )
    .map_err(ComputationInitError::Registration)?;
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Effect,
            phase,
            parent,
            computation: Box::new(computation),
        },
    );
    finish_creation(state, result, errors)
}

pub(crate) fn create_previous<'scope, T, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    phase: EffectPhase,
    mut callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<NodeId, E>
where
    T: 'scope,
    E: 'scope,
    F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
{
    let handler = match handler.lease() {
        Ok(handler) => handler,
        Err(error) => {
            return Err(ComputationInitError::Registration(ReactiveError::Handler(
                error,
            )));
        }
    };
    let value = storage.alloc_empty_slot::<T>();
    let errors = storage.alloc_error_slot();
    let evaluator_errors = errors.clone();
    let evaluator: ComputedEvaluator<'scope, T> = Box::new(move |previous, _scheduler| {
        callback(previous)
            .map(|value| ComputedEvaluation {
                value,
                stop_after_run: false,
            })
            .map_err(|error| {
                ComputationExecutionError::Callback(ErrorEvent::new(
                    error,
                    &handler,
                    evaluator_errors.clone(),
                ))
            })
    });
    let computation = ComputedNode::<T, E>::new(
        Some(value),
        errors.reference(),
        evaluator,
        Box::new(|_, _| true),
        false,
    )
    .map_err(ComputationInitError::Registration)?;
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Previous,
            phase,
            parent: ComputationParent::Current,
            computation: Box::new(computation),
        },
    );
    finish_creation(state, result, errors)
}

pub(crate) fn create_watch<'scope, T, E, G, C>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    phase: EffectPhase,
    mut getter: G,
    mut callback: C,
    handler: ErrorHandlerRef<'scope, E>,
    options: WatchOptions,
) -> ComputationInitResult<NodeId, E>
where
    T: PartialEq + 'scope,
    E: 'scope,
    G: FnMut() -> Result<T, E> + 'scope,
    C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
{
    let handler = match handler.lease() {
        Ok(handler) => handler,
        Err(error) => {
            return Err(ComputationInitError::Registration(ReactiveError::Handler(
                error,
            )));
        }
    };
    let value = storage.alloc_empty_slot::<T>();
    let errors = storage.alloc_error_slot();
    let evaluator_errors = errors.clone();
    let evaluator: ComputedEvaluator<'scope, T> = Box::new(move |previous, scheduler| {
        let new = getter().map_err(|error| {
            ComputationExecutionError::Callback(ErrorEvent::new(
                error,
                &handler,
                evaluator_errors.clone(),
            ))
        })?;
        let first_run = previous.is_none();
        let changed = first_run || previous.is_none_or(|old| *old != new);
        let should_callback = if first_run {
            options.immediate
        } else {
            changed
        };
        if should_callback {
            let callback_result = {
                let _observer_frame = ObserverFrame::push_untracked(scheduler)
                    .map_err(ComputationExecutionError::Runtime)?;
                callback(&new, previous)
            };
            callback_result.map_err(|error| {
                ComputationExecutionError::Callback(ErrorEvent::new(
                    error,
                    &handler,
                    evaluator_errors.clone(),
                ))
            })?;
        }
        Ok(ComputedEvaluation {
            value: new,
            stop_after_run: should_callback && options.once,
        })
    });
    let computation = ComputedNode::<T, E>::new(
        Some(value),
        errors.reference(),
        evaluator,
        Box::new(|old, new| old.is_none_or(|old| *old != *new)),
        false,
    )
    .map_err(ComputationInitError::Registration)?;
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Watch,
            phase,
            parent: ComputationParent::Current,
            computation: Box::new(computation),
        },
    );
    finish_creation(state, result, errors)
}

pub(crate) fn create_computed<'scope, T, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<TypedComputation<'scope, T, E>, E>
where
    T: PartialEq + 'scope,
    E: 'scope,
    F: FnMut() -> Result<T, E> + 'scope,
{
    create_computed_with_policy(
        storage,
        state,
        callback,
        handler,
        Box::new(|old, new| old.is_none_or(|old| *old != *new)),
    )
}

pub(crate) fn create_computed_always<'scope, T, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<TypedComputation<'scope, T, E>, E>
where
    T: 'scope,
    E: 'scope,
    F: FnMut() -> Result<T, E> + 'scope,
{
    create_computed_with_policy(storage, state, callback, handler, Box::new(|_, _| true))
}

fn create_computed_with_policy<'scope, T, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    mut callback: F,
    handler: ErrorHandlerRef<'scope, E>,
    changed: ChangePredicate<'scope, T>,
) -> ComputationInitResult<TypedComputation<'scope, T, E>, E>
where
    T: 'scope,
    E: 'scope,
    F: FnMut() -> Result<T, E> + 'scope,
{
    let handler = match handler.lease() {
        Ok(handler) => handler,
        Err(error) => {
            return Err(ComputationInitError::Registration(ReactiveError::Handler(
                error,
            )));
        }
    };
    let value = storage.alloc_empty_slot::<T>();
    let errors = storage.alloc_error_slot();
    let errors_ref = errors.reference();
    let evaluator_errors = errors.clone();
    let evaluator: ComputedEvaluator<'scope, T> = Box::new(move |_previous, _scheduler| {
        callback()
            .map(|value| ComputedEvaluation {
                value,
                stop_after_run: false,
            })
            .map_err(|error| {
                ComputationExecutionError::Callback(ErrorEvent::new(
                    error,
                    &handler,
                    evaluator_errors.clone(),
                ))
            })
    });
    let computation =
        ComputedNode::<T, E>::new(Some(value), errors.reference(), evaluator, changed, true)
            .map_err(ComputationInitError::Registration)?;
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Computed,
            phase: EffectPhase::Normal,
            parent: ComputationParent::Current,
            computation: Box::new(computation),
        },
    );
    finish_creation(state, result, errors).and_then(|raw| {
        let value = state
            .try_borrow()
            .map_err(ComputationInitError::Registration)?
            .typed_node_ref(raw)
            .map_err(ComputationInitError::Registration)?;
        Ok(TypedComputation {
            raw,
            value,
            errors: errors_ref,
        })
    })
}
