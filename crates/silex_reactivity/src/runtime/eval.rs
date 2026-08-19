//! Computation evaluation engine and queue flush scheduler.

use super::{
    dispose::{dispatch_cleanup_errors, dispose_nodes, dispose_nodes_collect, run_cleanups},
    model::{NodeState, ScopeState},
    scheduler::{GlobalScheduler, Observer, ObserverFrame, OwnerId, TargetNode},
    storage::{CleanupThunk, NodeStorage},
};
use crate::{
    ReactiveError, ReactiveResult,
    borrow::SharedCell,
    error::{ErrorContext, ErrorEvent, ErrorPhase, HandlerError},
    handle::NodeKindTag,
    internal::NodeId,
};
use slotmap::Key;
use std::{
    any::Any,
    cell::Cell,
    fmt, mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

#[cfg(miri)]
// Keep the non-convergence regression bounded under the interpreter; the
// production budget below remains unchanged.
const MAX_QUEUE_ITERATIONS: usize = 10;

#[cfg(not(miri))]
const MAX_QUEUE_ITERATIONS: usize = 100_000;

type PanicData = Box<dyn Any + Send>;

fn node_error_context(state: &ScopeState<'_>, id: NodeId, phase: &'static str) -> ErrorContext {
    let Ok(state_ref) = state.try_borrow() else {
        return ErrorContext::new(phase);
    };
    let Some(node) = state_ref.nodes.get(id) else {
        return ErrorContext::new(phase);
    };
    let node_kind = match node.kind {
        NodeKindTag::Signal => "signal",
        NodeKindTag::Computed => "computed",
        NodeKindTag::Effect => "effect",
        NodeKindTag::Stored => "stored",
        NodeKindTag::Callback => "callback",
        NodeKindTag::NodeRef => "node ref",
    };
    ErrorContext {
        owner: Some(state_ref.owner_id.0),
        node_kind: Some(node_kind),
        node_id: Some(id.data().as_ffi()),
        phase,
    }
}

struct ComputationResult {
    commit_value: bool,
    notify: bool,
    stop_after_run: bool,
}

pub(crate) enum EvaluationError<'scope> {
    Runtime(ReactiveError),
    Callback(ErrorEvent<'scope>),
    Handler(HandlerError),
    User,
}

impl<'scope> From<ReactiveError> for EvaluationError<'scope> {
    fn from(error: ReactiveError) -> Self {
        Self::Runtime(error)
    }
}

impl fmt::Display for EvaluationError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => error.fmt(f),
            Self::Callback(_) => f.write_str("callback returned an error"),
            Self::Handler(error) => error.fmt(f),
            Self::User => f.write_str("callback returned a user error"),
        }
    }
}

type EvaluationResult<'scope, T> = Result<T, EvaluationError<'scope>>;

#[derive(Clone, Copy)]
enum EvaluationMode {
    Initial,
    Read,
    Deferred,
}

pub(crate) fn prepare_read<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    track: bool,
) -> ReactiveResult<()> {
    let tracking = if track {
        Some(
            state
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .preflight_track_read(id)?,
        )
    } else {
        None
    };
    let (settled, running) = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !state_ref.try_is_active()? {
            return Err(ReactiveError::NoSuchNode);
        }
        let node = state_ref.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        (state_ref.is_settled(id)?, node.running)
    };
    if running {
        return Err(ReactiveError::Reentrant);
    }
    if !settled {
        evaluate_root(state, id, EvaluationMode::Deferred).map_err(|error| match error {
            EvaluationError::Runtime(error) => error,
            EvaluationError::Callback(_) => ReactiveError::InvariantViolation,
            EvaluationError::Handler(error) => ReactiveError::Handler(error),
            EvaluationError::User => ReactiveError::InvariantViolation,
        })?;
    }
    if let Some(Some(ctx)) = tracking {
        state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .track_read(id, &ctx)?;
    }
    flush_if_idle(state)?;
    Ok(())
}

pub(crate) fn prepare_fallible_read<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    track: bool,
) -> EvaluationResult<'scope, ()> {
    let tracking = if track {
        Some(
            state
                .try_borrow()
                .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
                .preflight_track_read(id)
                .map_err(EvaluationError::Runtime)?,
        )
    } else {
        None
    };
    let (settled, running) = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
        if !state_ref.try_is_active()? {
            return Err(EvaluationError::Runtime(ReactiveError::NoSuchNode));
        }
        let node = state_ref
            .nodes
            .get(id)
            .ok_or(EvaluationError::Runtime(ReactiveError::NoSuchNode))?;
        (
            state_ref.is_settled(id).map_err(EvaluationError::Runtime)?,
            node.running,
        )
    };
    if running {
        return Err(EvaluationError::Runtime(ReactiveError::Reentrant));
    }
    if !settled {
        evaluate_root(state, id, EvaluationMode::Read)?;
    }
    if let Some(Some(ctx)) = tracking {
        state
            .try_borrow_mut()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
            .track_read(id, &ctx)
            .map_err(EvaluationError::Runtime)?;
    }
    flush_if_idle(state).map_err(EvaluationError::Runtime)?;
    Ok(())
}

fn evaluate_root<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    mode: EvaluationMode,
) -> EvaluationResult<'scope, ()> {
    let scheduler = {
        let state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let scheduler = state_ref.scheduler.clone();
        let mut sched = scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        sched.evaluating = sched.evaluating.saturating_add(1);
        scheduler.clone()
    };
    let mut stack = Vec::new();
    let result = catch_unwind(AssertUnwindSafe(|| evaluate(state, id, &mut stack, mode)));
    {
        let mut sched = scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        sched.evaluating = sched.evaluating.saturating_sub(1);
    }
    match result {
        Ok(result) => {
            flush_if_idle(state).map_err(EvaluationError::Runtime)?;
            result
        }
        Err(panic) => resume_unwind(panic),
    }
}

fn evaluate<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    stack: &mut Vec<TargetNode>,
    mode: EvaluationMode,
) -> EvaluationResult<'scope, ()> {
    let target = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        TargetNode {
            owner_id: state_ref.owner_id,
            node: id,
        }
    };
    let (node_state, running, dependencies) = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let node = state_ref.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        let deps: Vec<TargetNode> = state_ref
            .dependency_edges_of(id)
            .map(|(_, edge)| edge.target)
            .collect();
        (node.state, node.running, deps)
    };
    if node_state == NodeState::Clean || running {
        return Ok(());
    }
    if stack.contains(&target) {
        return Err(EvaluationError::Runtime(ReactiveError::Reentrant));
    }
    stack.push(target);
    for dep in &dependencies {
        if dep.owner_id
            == state
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .owner_id
        {
            let dependency_state = state
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .nodes
                .get(dep.node)
                .map(|node| node.state);
            if dependency_state.is_some_and(|state| state != NodeState::Clean) {
                evaluate(state, dep.node, stack, mode)?;
            }
        } else {
            let scheduler = state
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .scheduler
                .clone();
            let dep_scope = scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .get_scope(dep.owner_id)
                .map_err(EvaluationError::Runtime)?;
            if let Some(dep_scope) = dep_scope {
                let dependency_state = dep_scope
                    .try_borrow()
                    .map_err(|_| ReactiveError::BorrowConflict)?
                    .nodes
                    .get(dep.node)
                    .map(|node| node.state);
                if dependency_state.is_some_and(|state| state != NodeState::Clean) {
                    evaluate(&dep_scope, dep.node, stack, mode)?;
                }
            }
        }
    }
    stack.pop();

    let skip = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let Some(node) = state_ref.nodes.get(id) else {
            return Err(EvaluationError::Runtime(ReactiveError::NoSuchNode));
        };
        if node.state == NodeState::Check {
            let scheduler = state_ref.scheduler.clone();
            let dependency_epochs = state_ref
                .dependency_edges_of(id)
                .map(|(_, edge)| -> EvaluationResult<'scope, u64> {
                    let dep = edge.target;
                    if dep.owner_id == state_ref.owner_id {
                        Ok(state_ref
                            .nodes
                            .get(dep.node)
                            .map(|target| target.updated_epoch)
                            .unwrap_or(0))
                    } else {
                        let dep_scope = scheduler
                            .try_borrow()
                            .map_err(|_| ReactiveError::BorrowConflict)?
                            .get_scope(dep.owner_id)?;
                        let Some(dep_scope) = dep_scope else {
                            return Ok(0);
                        };
                        let dep_state = dep_scope
                            .try_borrow()
                            .map_err(|_| ReactiveError::BorrowConflict)?;
                        Ok(dep_state
                            .nodes
                            .get(dep.node)
                            .map(|node| node.updated_epoch)
                            .unwrap_or(0))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let max_dep_updated_epoch = dependency_epochs.into_iter().max().unwrap_or(0);
            node.last_computed_epoch >= max_dep_updated_epoch
        } else {
            false
        }
    };
    if skip {
        let mut state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let current_epoch = state_ref
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .current_epoch();
        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.state = NodeState::Clean;
            node.last_computed_epoch = current_epoch;
        }
        return Ok(());
    }
    if run_node(state, id, mode)? {
        Ok(())
    } else {
        Err(EvaluationError::Runtime(ReactiveError::NoSuchNode))
    }
}

fn execute_computation<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    storage: &NodeStorage<'scope>,
    scheduler: SharedCell<GlobalScheduler>,
) -> EvaluationResult<'scope, ComputationResult> {
    let NodeStorage::Computation(computation) = storage else {
        return Err(EvaluationError::Runtime(ReactiveError::WrongKind));
    };

    let mut computation_lease = computation
        .computation
        .try_write(scheduler.clone())
        .map_err(EvaluationError::Runtime)?;
    let mut behavior = mem::take(&mut *computation_lease)
        .ok_or(EvaluationError::Runtime(ReactiveError::NoSuchNode))?;
    drop(computation_lease);

    let result = catch_unwind(AssertUnwindSafe(|| behavior.execute(scheduler.clone())));
    let should_restore = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
        state_ref
            .try_is_active()
            .map_err(EvaluationError::Runtime)?
            && state_ref.node_exists(id)
    };
    let restore_result = if should_restore {
        match computation.computation.try_write(scheduler.clone()) {
            Ok(mut computation_lease) => {
                if computation_lease.is_some() {
                    match behavior.clear() {
                        Ok(()) => Err(ReactiveError::BorrowConflict),
                        Err(error) => Err(error),
                    }
                } else {
                    *computation_lease = Some(behavior);
                    Ok(())
                }
            }
            Err(error) => match behavior.clear() {
                Ok(()) => Err(error),
                Err(clear_error) => Err(clear_error),
            },
        }
    } else {
        behavior.clear()
    };

    let result = match result {
        Ok(result) => {
            restore_result.map_err(EvaluationError::Runtime)?;
            result
        }
        Err(panic) => resume_unwind(panic),
    };
    let result = match result {
        Ok(result) => result,
        Err(super::storage::ComputationExecutionError::Runtime(error)) => {
            return Err(EvaluationError::Runtime(error));
        }
        Err(super::storage::ComputationExecutionError::Callback(error)) => {
            return Err(EvaluationError::Callback(error));
        }
    };
    Ok(ComputationResult {
        commit_value: result.commit_value,
        notify: result.notify,
        stop_after_run: result.stop_after_run,
    })
}

fn commit_computation_value<'scope>(
    storage: &NodeStorage<'scope>,
    scheduler: SharedCell<GlobalScheduler>,
) -> ReactiveResult<()> {
    let NodeStorage::Computation(computation) = storage else {
        return Err(ReactiveError::WrongKind);
    };
    let mut computation_lease = computation.computation.try_write(scheduler.clone())?;
    computation_lease
        .as_mut()
        .as_mut()
        .ok_or(ReactiveError::NoSuchNode)?
        .commit(scheduler)
}

fn discard_computation_pending<'scope>(
    storage: &NodeStorage<'scope>,
    scheduler: SharedCell<GlobalScheduler>,
) -> ReactiveResult<()> {
    let NodeStorage::Computation(computation) = storage else {
        return Err(ReactiveError::WrongKind);
    };
    let mut computation_lease = computation.computation.try_write(scheduler)?;
    computation_lease
        .as_mut()
        .as_mut()
        .ok_or(ReactiveError::NoSuchNode)?
        .discard_pending();
    Ok(())
}

fn remember_panic(first: &mut Option<PanicData>, panic: PanicData) {
    if first.is_none() {
        *first = Some(panic);
    }
}

fn drop_storage<'scope>(
    scheduler: SharedCell<GlobalScheduler>,
    storage: Rc<NodeStorage<'scope>>,
) -> Option<PanicData> {
    let _observer_frame = ObserverFrame::push_untracked(scheduler);
    catch_unwind(AssertUnwindSafe(|| drop(storage))).err()
}

fn run_node<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
    mode: EvaluationMode,
) -> EvaluationResult<'scope, bool> {
    struct RunningNodeContext<'scope> {
        storage: Rc<NodeStorage<'scope>>,
        first_child: NodeId,
        cleanups: Vec<CleanupThunk<'scope>>,
        previous_owner: Option<NodeId>,
        scheduler: SharedCell<GlobalScheduler>,
        owner_id: OwnerId,
    }

    let node_ctx = {
        let mut state_ref = state
            .try_borrow_mut()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
        let Some(node) = state_ref.nodes.get(id) else {
            return Ok(false);
        };
        if !node.is_computation() || node.running {
            return Ok(false);
        }
        let first_child = node.first_child;
        let Some(data) = state_ref.data.get_mut(id) else {
            return Ok(false);
        };
        if !matches!(data.storage.as_ref(), NodeStorage::Computation(_)) {
            return Ok(false);
        }
        let storage = data.storage.clone();
        let cleanups = mem::take(&mut data.cleanups);
        let previous_owner = state_ref.current_owner;
        let scheduler = state_ref.scheduler.clone();
        let owner_id = state_ref.owner_id;
        state_ref.begin_dependency_transaction(id);
        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.running = true;
            node.first_child = NodeId::DANGLING;
        }
        state_ref.current_owner = Some(id);
        RunningNodeContext {
            storage,
            first_child,
            cleanups,
            previous_owner,
            scheduler,
            owner_id,
        }
    };

    let RunningNodeContext {
        storage,
        first_child,
        cleanups,
        previous_owner,
        scheduler,
        owner_id,
    } = node_ctx;

    let children_to_dispose: Vec<NodeId> = state
        .try_borrow()
        .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
        .children_of_head(first_child)
        .collect();
    let mut execution_started = false;
    let mut observer_frame = None;
    let mut cleanup_errors = Vec::new();
    let outcome = catch_unwind(AssertUnwindSafe(
        || -> EvaluationResult<'scope, ComputationResult> {
            let child_dispose = catch_unwind(AssertUnwindSafe(|| {
                dispose_nodes_collect(state, children_to_dispose)
            }));
            let mut cleanup_panic = match child_dispose {
                Ok(Ok(mut child_outcome)) => {
                    cleanup_errors.extend(child_outcome.errors);
                    child_outcome.panics.pop()
                }
                Ok(Err(error)) => return Err(EvaluationError::Runtime(error)),
                Err(panic) => Some(panic),
            };
            let cleanup_outcome = run_cleanups(scheduler.clone(), cleanups);
            cleanup_errors.extend(cleanup_outcome.errors);
            if cleanup_panic.is_none() {
                cleanup_panic = cleanup_outcome.panics.into_iter().next();
            }
            if let Some(panic) = cleanup_panic {
                resume_unwind(panic);
            }

            {
                let mut state_ref = state
                    .try_borrow_mut()
                    .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
                if !state_ref.node_exists(id) {
                    return Ok(ComputationResult {
                        commit_value: false,
                        notify: false,
                        stop_after_run: false,
                    });
                }
                let scheduler = state_ref.scheduler.clone();
                let mut scheduler_ref = scheduler
                    .try_borrow_mut()
                    .map_err(|_| ReactiveError::BorrowConflict)?;
                scheduler_ref.executing = scheduler_ref.executing.saturating_add(1);
                drop(scheduler_ref);
                execution_started = true;
                state_ref.current_owner = Some(id);
                observer_frame = Some(ObserverFrame::push(
                    scheduler,
                    Some(Observer {
                        owner_id: state_ref.owner_id,
                        node: id,
                    }),
                )?);
                if let Some(node) = state_ref.nodes.get_mut(id) {
                    node.state = NodeState::Clean;
                }
            }

            execute_computation(state, id, &storage, scheduler.clone())
        },
    ));

    drop(observer_frame);

    let mut panic_data = None;
    let mut operation_error: Option<EvaluationError<'scope>> = None;
    let mut result = None;
    match outcome {
        Ok(Ok(value)) => result = Some(value),
        Ok(Err(error)) => operation_error = Some(error),
        Err(panic) => panic_data = Some(panic),
    }

    let mut notify = false;
    let mut committed = false;
    let mut stop_after_run = false;
    let mut transaction_finished = false;
    let can_commit = if operation_error.is_none() && panic_data.is_none() {
        match state.try_borrow() {
            Ok(state_ref) => {
                state_ref.node_exists(id)
                    && state_ref.try_is_active()?
                    && state_ref.owner_id == owner_id
            }
            Err(_) => {
                operation_error = Some(EvaluationError::Runtime(ReactiveError::BorrowConflict));
                false
            }
        }
    } else {
        false
    };

    if can_commit && let Some(computation_result) = result.as_mut() {
        notify = computation_result.notify;
        stop_after_run = computation_result.stop_after_run;
        let commit_dependencies = catch_unwind(AssertUnwindSafe(|| {
            state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .commit_dependency_transaction(id)?;
            Ok::<(), ReactiveError>(())
        }));
        match commit_dependencies {
            Ok(Ok(())) => {
                if computation_result.commit_value {
                    let commit = catch_unwind(AssertUnwindSafe(|| {
                        let _observer_frame = ObserverFrame::push_untracked(scheduler.clone())?;
                        commit_computation_value(&storage, scheduler.clone())
                    }));
                    match commit {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            operation_error = Some(EvaluationError::Runtime(error));
                            committed = false;
                        }
                        Err(panic) => {
                            panic_data = Some(panic);
                            committed = false;
                        }
                    }
                }
                if operation_error.is_none() && panic_data.is_none() {
                    let finish = catch_unwind(AssertUnwindSafe(|| {
                        state
                            .try_borrow_mut()
                            .map_err(|_| ReactiveError::BorrowConflict)?
                            .finish_dependency_transaction(id);
                        Ok::<(), ReactiveError>(())
                    }));
                    match finish {
                        Ok(Ok(())) => {
                            transaction_finished = true;
                            committed = true;
                        }
                        Ok(Err(error)) => {
                            operation_error = Some(EvaluationError::Runtime(error));
                        }
                        Err(panic) => panic_data = Some(panic),
                    }
                }
            }
            Ok(Err(error)) => {
                operation_error = Some(EvaluationError::Runtime(error));
            }
            Err(panic) => panic_data = Some(panic),
        }
    }

    let failed = operation_error.is_some() || panic_data.is_some() || !committed;
    if !transaction_finished {
        let rollback = catch_unwind(AssertUnwindSafe(|| {
            state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .rollback_dependency_transaction(id)?;
            Ok::<(), ReactiveError>(())
        }));
        match rollback {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                operation_error = Some(EvaluationError::Runtime(error));
            }
            Err(panic) => remember_panic(&mut panic_data, panic),
        }
    }

    let mut failed_children = Vec::new();
    let mut failed_cleanups = Vec::new();
    {
        let mut state_ref = state
            .try_borrow_mut()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?;
        let now_epoch = state_ref
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .current_epoch();
        if execution_started {
            let mut scheduler = state_ref
                .scheduler
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?;
            scheduler.executing = scheduler.executing.saturating_sub(1);
        }
        state_ref.set_ctx(previous_owner);
        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.running = false;
            node.last_computed_epoch = now_epoch;
            if failed {
                node.state = NodeState::Dirty;
            } else if notify && committed {
                node.updated_epoch = now_epoch;
                node.version = node.version.wrapping_add(1);
                state_ref
                    .queue_dependents(id)
                    .map_err(EvaluationError::Runtime)?;
            }
        }
        if failed {
            if let Some(node) = state_ref.nodes.get_mut(id) {
                let first_child = node.first_child;
                node.first_child = NodeId::DANGLING;
                failed_children = state_ref.children_of_head(first_child).collect();
            }
            if let Some(data) = state_ref.data.get_mut(id) {
                failed_cleanups = mem::take(&mut data.cleanups);
            }
        }
    }

    if failed {
        let node_still_exists = match state.try_borrow() {
            Ok(state_ref) => state_ref.node_exists(id),
            Err(_) => {
                operation_error = Some(EvaluationError::Runtime(ReactiveError::BorrowConflict));
                false
            }
        };
        if node_still_exists {
            let discard = catch_unwind(AssertUnwindSafe(|| {
                discard_computation_pending(&storage, scheduler.clone())
            }));
            match discard {
                Ok(Ok(())) => {}
                Ok(Err(error)) => operation_error = Some(EvaluationError::Runtime(error)),
                Err(panic) => remember_panic(&mut panic_data, panic),
            }
        }
        let child_dispose = catch_unwind(AssertUnwindSafe(|| {
            dispose_nodes_collect(state, failed_children)
        }));
        match child_dispose {
            Ok(Ok(outcome)) => {
                cleanup_errors.extend(outcome.errors);
                for panic in outcome.panics {
                    remember_panic(&mut panic_data, panic);
                }
            }
            Ok(Err(error)) => operation_error = Some(EvaluationError::Runtime(error)),
            Err(panic) => remember_panic(&mut panic_data, panic),
        }
        let cleanup_outcome = run_cleanups(scheduler.clone(), failed_cleanups);
        cleanup_errors.extend(cleanup_outcome.errors);
        for panic in cleanup_outcome.panics {
            remember_panic(&mut panic_data, panic);
        }
    }

    if let Some(panic) = drop_storage(scheduler.clone(), storage) {
        remember_panic(&mut panic_data, panic);
    }

    let dispatch_outcome = dispatch_cleanup_errors(scheduler.clone(), cleanup_errors);
    for panic in dispatch_outcome.panics {
        remember_panic(&mut panic_data, panic);
    }
    if let Some(error) = dispatch_outcome.handler_errors.into_iter().next() {
        operation_error = Some(EvaluationError::Handler(error));
    }

    if stop_after_run && !failed && committed {
        let stop_result = catch_unwind(AssertUnwindSafe(|| dispose_nodes(state, vec![id])));
        match stop_result {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => operation_error = Some(EvaluationError::Runtime(error)),
            Err(panic) => remember_panic(&mut panic_data, panic),
        }
    }
    if let Some(panic) = panic_data {
        resume_unwind(panic);
    }
    if let Some(error) = operation_error {
        match (mode, error) {
            (EvaluationMode::Deferred, EvaluationError::Callback(error)) => {
                match error.dispatch_with_context(
                    ErrorPhase::Deferred,
                    node_error_context(state, id, "deferred"),
                ) {
                    Ok(()) => {
                        flush_if_idle(state).map_err(EvaluationError::Runtime)?;
                        return Ok(true);
                    }
                    Err(error) => return Err(EvaluationError::Handler(error)),
                }
            }
            (EvaluationMode::Read, EvaluationError::Callback(error)) => {
                error
                    .dispatch_with_context(ErrorPhase::Read, node_error_context(state, id, "read"))
                    .map_err(EvaluationError::Handler)?;
                return Err(EvaluationError::User);
            }
            (_, error) => return Err(error),
        }
    }
    flush_if_idle(state).map_err(EvaluationError::Runtime)?;
    Ok(true)
}

pub(crate) fn run_initial<'scope>(
    state: &ScopeState<'scope>,
    id: NodeId,
) -> EvaluationResult<'scope, ()> {
    match run_node(state, id, EvaluationMode::Initial)? {
        true => Ok(()),
        false => Err(EvaluationError::Runtime(ReactiveError::NoSuchNode)),
    }
}

pub(crate) fn flush_if_idle<'scope>(state: &ScopeState<'scope>) -> ReactiveResult<()> {
    if let Ok(mut state_ref) = state.try_borrow_mut() {
        state_ref.sweep_error_handlers();
    }
    let scheduler = state.try_borrow()?.scheduler.clone();
    let should_flush = scheduler
        .try_borrow()
        .map_err(|_| ReactiveError::BorrowConflict)?
        .should_flush();
    if should_flush {
        run_global_queue(&scheduler)?;
    }
    Ok(())
}

pub(crate) fn run_global_queue(scheduler: &SharedCell<GlobalScheduler>) -> ReactiveResult<()> {
    let acquired = {
        let mut sched = scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if sched.running_queue {
            false
        } else {
            sched.running_queue = true;
            let incoming = mem::take(&mut sched.global_queue);
            sched.worklist.extend(incoming);
            true
        }
    };
    if !acquired {
        return Ok(());
    }
    let mut guard = QueueRunGuard::new(scheduler.clone());
    let outcome = catch_unwind(AssertUnwindSafe(|| -> ReactiveResult<()> {
        let mut iterations: usize = 0;
        loop {
            let next_task = scheduler
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .worklist
                .pop_front();
            let next_task = match next_task {
                Some(task) => Some(task),
                None => {
                    let mut scheduler_ref = scheduler
                        .try_borrow_mut()
                        .map_err(|_| ReactiveError::BorrowConflict)?;
                    if scheduler_ref.global_queue.is_empty() {
                        None
                    } else {
                        let incoming = mem::take(&mut scheduler_ref.global_queue);
                        scheduler_ref.worklist.extend(incoming);
                        scheduler_ref.worklist.pop_front()
                    }
                }
            };
            let Some(task) = next_task else {
                break;
            };
            guard.failed_owner.set(Some(task.owner_id));
            if !scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .is_scope_active(task.owner_id)
            {
                guard.failed_owner.set(None);
                continue;
            }
            iterations = iterations.saturating_add(1);
            if iterations > MAX_QUEUE_ITERATIONS {
                return Err(ReactiveError::NonConvergent {
                    iterations,
                    last_scope: Some(task.owner_id.0),
                    last_node: Some(task.node.data().as_ffi()),
                });
            }
            let scope_state = scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .get_scope(task.owner_id)?;
            if let Some(scope_state) = scope_state {
                {
                    let mut state_ref = scope_state
                        .try_borrow_mut()
                        .map_err(|_| ReactiveError::BorrowConflict)?;
                    if let Some(node) = state_ref.nodes.get_mut(task.node) {
                        node.queued = false;
                    }
                }
                evaluate_root(&scope_state, task.node, EvaluationMode::Deferred).map_err(
                    |error| match error {
                        EvaluationError::Runtime(error) => error,
                        EvaluationError::Handler(error) => ReactiveError::Handler(error),
                        EvaluationError::Callback(_) | EvaluationError::User => {
                            ReactiveError::InvariantViolation
                        }
                    },
                )?;
            }
            guard.failed_owner.set(None);
        }
        Ok(())
    }));

    match outcome {
        Ok(Ok(())) => guard.finish(),
        Ok(Err(error)) => {
            drop(guard);
            Err(error)
        }
        Err(panic) => {
            drop(guard);
            resume_unwind(panic)
        }
    }
}

struct QueueRunGuard {
    scheduler: SharedCell<GlobalScheduler>,
    failed_owner: Cell<Option<OwnerId>>,
    finished: bool,
}

impl QueueRunGuard {
    fn new(scheduler: SharedCell<GlobalScheduler>) -> Self {
        Self {
            scheduler,
            failed_owner: Cell::new(None),
            finished: false,
        }
    }

    fn finish(&mut self) -> ReactiveResult<()> {
        let mut scheduler = self
            .scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        scheduler.worklist.clear();
        scheduler.running_queue = false;
        self.finished = true;
        Ok(())
    }
}

impl Drop for QueueRunGuard {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        recover_after_queue_error(&self.scheduler, self.failed_owner.get());
        if let Ok(mut scheduler) = self.scheduler.try_borrow_mut() {
            scheduler.running_queue = false;
        }
    }
}

fn recover_after_queue_error(
    scheduler: &SharedCell<GlobalScheduler>,
    failed_owner: Option<OwnerId>,
) {
    if let Ok(mut scheduler_ref) = scheduler.try_borrow_mut()
        && let Some(owner_id) = failed_owner
    {
        scheduler_ref
            .worklist
            .retain(|task| task.owner_id != owner_id);
        scheduler_ref
            .global_queue
            .retain(|task| task.owner_id != owner_id);
    }

    let Some(owner_id) = failed_owner else {
        return;
    };
    let scheduler_ref = match scheduler.try_borrow() {
        Ok(scheduler_ref) => scheduler_ref,
        Err(_) => return,
    };
    let scope_state = match scheduler_ref.resolve_cleanup_owner(owner_id) {
        Ok(Some(proof)) => proof.state(),
        Ok(None) | Err(_) => return,
    };
    let Ok(mut state) = scope_state.try_borrow_mut() else {
        return;
    };
    for node in state.nodes.values_mut() {
        if node.state == NodeState::Check {
            node.state = NodeState::Dirty;
        }
        node.queued = false;
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]

    use super::*;
    use crate::{
        ErrorHandlerToken,
        owner::{OwnerAccess, ScopeStorage},
        runtime::model::NodeState,
        runtime::scheduler::ScheduledTask,
    };
    use std::{cell::Cell, marker::PhantomData};

    fn handler<'scope>(owner: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
        owner.error_handler(|_| {}).expect("handler registration")
    }

    #[test]
    fn queue_error_retains_other_owner_worklist_and_incoming_tasks() {
        let scheduler = GlobalScheduler::new();
        let failed_storage = ScopeStorage::new(scheduler.clone()).expect("failed owner setup");
        let retained_storage = ScopeStorage::new(scheduler.clone()).expect("retained owner setup");
        let retained_scope = OwnerAccess {
            storage: &retained_storage,
            marker: PhantomData,
        };
        let first_runs = std::rc::Rc::new(Cell::new(0));
        let followup_runs = std::rc::Rc::new(Cell::new(0));
        let enqueue_followup = std::rc::Rc::new(Cell::new(false));
        let followup_id = std::rc::Rc::new(Cell::new(None));
        let retained_owner_id = retained_storage.owner_id;

        let followup_runs_in_effect = followup_runs.clone();
        let followup = retained_scope
            .effect(
                move || {
                    followup_runs_in_effect.set(followup_runs_in_effect.get() + 1);
                    Ok(())
                },
                handler(retained_scope),
            )
            .expect("follow-up effect creation");
        let followup_node = followup.handle.raw();
        followup_id.set(Some(followup_node));
        retained_scope
            .state()
            .try_borrow_mut()
            .expect("state write")
            .nodes
            .get_mut(followup_node)
            .expect("follow-up effect node")
            .state = NodeState::Dirty;

        let first_runs_in_effect = first_runs.clone();
        let enqueue_followup_in_effect = enqueue_followup.clone();
        let followup_id_in_effect = followup_id.clone();
        let scheduler_in_effect = scheduler.clone();
        let first = retained_scope
            .effect(
                move || {
                    first_runs_in_effect.set(first_runs_in_effect.get() + 1);
                    if enqueue_followup_in_effect.get() {
                        scheduler_in_effect
                            .try_borrow_mut()
                            .expect("scheduler write")
                            .global_queue
                            .push_back(ScheduledTask {
                                owner_id: retained_owner_id,
                                node: followup_id_in_effect.get().expect("follow-up node id"),
                            });
                    }
                    Ok(())
                },
                handler(retained_scope),
            )
            .expect("first effect creation");
        let first_node = first.handle.raw();
        retained_scope
            .state()
            .try_borrow_mut()
            .expect("state write")
            .nodes
            .get_mut(first_node)
            .expect("first effect node")
            .state = NodeState::Dirty;
        enqueue_followup.set(true);

        {
            let mut scheduler_ref = scheduler.try_borrow_mut().expect("scheduler write");
            scheduler_ref.global_queue.push_back(ScheduledTask {
                owner_id: retained_owner_id,
                node: first_node,
            });
            scheduler_ref.global_queue.push_back(ScheduledTask {
                owner_id: failed_storage.owner_id,
                node: NodeId::DANGLING,
            });
        }

        assert_eq!(run_global_queue(&scheduler), Err(ReactiveError::NoSuchNode));
        assert_eq!(first_runs.get(), 2);
        assert_eq!(followup_runs.get(), 1);
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .global_queue
                .len(),
            1
        );
        assert!(
            !scheduler
                .try_borrow()
                .expect("scheduler read")
                .running_queue
        );

        enqueue_followup.set(false);
        assert_eq!(run_global_queue(&scheduler), Ok(()));
        assert_eq!(followup_runs.get(), 2);
        assert!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .global_queue
                .is_empty()
        );
        assert!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .worklist
                .is_empty()
        );

        let _ = failed_storage.dispose_untracked();
        let _ = retained_storage.dispose_untracked();
    }
}
