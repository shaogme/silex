//! Computation creation kernel.

use super::{
    dispose_nodes,
    eval::{self, EvaluationError, flush_if_idle},
    model::ScopeState,
    scheduler::InitialFlushGuard,
    storage::{
        ComputationBehavior, DerivedBehavior, EffectBehavior, MemoBehavior, PreviousBehavior,
        TypedNodeRef, WatchBehavior,
    },
};
use crate::{
    ComputationInitError, ComputationInitResult, ErrorHandlerRef, ReactiveError,
    child::WatchOptions,
    error::{ErrorPhase, ErrorSlot},
    handle::NodeKindTag,
    internal::RawId,
    scope::ScopeStorage,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Clone, Copy)]
pub(crate) enum ComputationKind {
    Effect,
    Previous,
    Watch,
    Memo,
    Derived,
}

impl ComputationKind {
    fn tag(self) -> NodeKindTag {
        match self {
            Self::Effect | Self::Previous | Self::Watch => NodeKindTag::Effect,
            Self::Memo => NodeKindTag::Memo,
            Self::Derived => NodeKindTag::Derived,
        }
    }
}

pub(crate) struct ComputationSpec<'scope> {
    pub(crate) kind: ComputationKind,
    pub(crate) computation: Box<dyn ComputationBehavior<'scope> + 'scope>,
}

pub(crate) struct TypedComputation<'scope, T, E> {
    pub(crate) raw: RawId,
    pub(crate) value: TypedNodeRef<'scope, T>,
    pub(crate) errors: &'scope ErrorSlot<E>,
}

pub(crate) fn create_computation<'scope>(
    state: &ScopeState<'scope>,
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

    let raw = {
        let mut state = state
            .try_borrow_mut()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
        state
            .register_computation(spec.kind.tag(), spec.computation)
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
    result: Result<RawId, EvaluationError<'scope>>,
    errors: &'scope ErrorSlot<E>,
) -> ComputationInitResult<RawId, E> {
    match result {
        Ok(raw) => Ok(raw),
        Err(EvaluationError::Runtime(error)) => Err(ComputationInitError::Registration(error)),
        Err(EvaluationError::Callback(error)) => {
            error.dispatch(ErrorPhase::Initial).map_err(|error| {
                ComputationInitError::Registration(ReactiveError::Handler(error))
            })?;
            let initial = errors.take();
            flush_if_idle(state).map_err(ComputationInitError::Registration)?;
            Err(ComputationInitError::Initial(initial))
        }
        Err(EvaluationError::Handler(error)) => Err(ComputationInitError::Registration(
            ReactiveError::Handler(error),
        )),
        Err(EvaluationError::User) => Err(ComputationInitError::Registration(
            ReactiveError::TypeMismatch,
        )),
    }
}

pub(crate) fn create_effect<'scope, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<RawId, E>
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
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Effect,
            computation: Box::new(EffectBehavior::new(callback, handler, errors)),
        },
    );
    finish_creation(state, result, errors)
}

pub(crate) fn create_previous<'scope, T, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<RawId, E>
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
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Previous,
            computation: Box::new(PreviousBehavior::new(
                value.slot(),
                callback,
                handler,
                errors,
            )),
        },
    );
    finish_creation(state, result, errors)
}

pub(crate) fn create_watch<'scope, T, E, G, C>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    getter: G,
    callback: C,
    handler: ErrorHandlerRef<'scope, E>,
    options: WatchOptions,
) -> ComputationInitResult<RawId, E>
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
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Watch,
            computation: Box::new(WatchBehavior::new(
                value.slot(),
                getter,
                callback,
                handler,
                errors,
                options.immediate,
                options.once,
            )),
        },
    );
    finish_creation(state, result, errors)
}

pub(crate) fn create_memo<'scope, T, E, F>(
    storage: &'scope ScopeStorage,
    state: &ScopeState<'scope>,
    callback: F,
    handler: ErrorHandlerRef<'scope, E>,
) -> ComputationInitResult<TypedComputation<'scope, T, E>, E>
where
    T: PartialEq + 'scope,
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
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Memo,
            computation: Box::new(MemoBehavior::new(value.slot(), callback, handler, errors)),
        },
    );
    finish_creation(state, result, errors).map(|raw| TypedComputation { raw, value, errors })
}

pub(crate) fn create_derived<'scope, T, E, F>(
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
    let result = create_computation(
        state,
        ComputationSpec {
            kind: ComputationKind::Derived,
            computation: Box::new(DerivedBehavior::new(
                value.slot(),
                callback,
                handler,
                errors,
            )),
        },
    );
    finish_creation(state, result, errors).map(|raw| TypedComputation { raw, value, errors })
}
