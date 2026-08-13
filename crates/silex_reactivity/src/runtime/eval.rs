//! Computation evaluation engine and queue flush scheduler.

use super::{
    dispose::{dispatch_cleanup_errors, dispose_nodes, dispose_nodes_collect, run_cleanups},
    model::{NodeState, ScopeState},
    scheduler::{GlobalScheduler, Observer, ObserverFrame, TargetNode},
    storage::NodeStorage,
};
use crate::{
    ReactiveError, ReactiveResult,
    error::{ErrorEvent, ErrorPhase},
    internal::{
        RawId,
        value::{AnyValue, Computation},
    },
};
use std::{
    any::Any,
    cell::RefCell,
    fmt, mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

const MAX_QUEUE_ITERATIONS: usize = 100_000;

type PanicData = Box<dyn Any + Send>;

struct ComputationResult<'scope> {
    produced_value: Option<AnyValue<'scope>>,
    commit_value: bool,
    notify: bool,
    stop_after_run: bool,
    initialize_watch: bool,
}

pub(crate) enum EvaluationError<'scope> {
    Runtime(ReactiveError),
    Callback(ErrorEvent<'scope>),
    User(AnyValue<'scope>),
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
            Self::User(_) => f.write_str("callback returned a user error"),
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
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
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
        if !state_ref.is_active() {
            return Err(ReactiveError::NoSuchNode);
        }
        let node = state_ref.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        (state_ref.is_settled(id), node.running)
    };
    if running {
        return Err(ReactiveError::Reentrant);
    }
    if !settled {
        evaluate_root(state, id, EvaluationMode::Deferred).map_err(|error| match error {
            EvaluationError::Runtime(error) => error,
            EvaluationError::Callback(_) => {
                unreachable!("deferred callback errors are consumed by their handler")
            }
            EvaluationError::User(_) => {
                unreachable!("user errors are only produced by fallible reads")
            }
        })?;
    }
    if let Some(Some(context)) = tracking {
        state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .track_read(id, &context)?;
    }
    flush_if_idle(state);
    Ok(())
}

pub(crate) fn prepare_fallible_read<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
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
        if !state_ref.is_active() {
            return Err(EvaluationError::Runtime(ReactiveError::NoSuchNode));
        }
        let node = state_ref
            .nodes
            .get(id)
            .ok_or(EvaluationError::Runtime(ReactiveError::NoSuchNode))?;
        (state_ref.is_settled(id), node.running)
    };
    if running {
        return Err(EvaluationError::Runtime(ReactiveError::Reentrant));
    }
    if !settled {
        evaluate_root(state, id, EvaluationMode::Read)?;
    }
    if let Some(Some(context)) = tracking {
        state
            .try_borrow_mut()
            .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
            .track_read(id, &context)
            .map_err(EvaluationError::Runtime)?;
    }
    flush_if_idle(state);
    Ok(())
}

fn evaluate_root<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    mode: EvaluationMode,
) -> EvaluationResult<'scope, ()> {
    let scheduler = {
        let state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let scheduler = state_ref.scheduler.clone();
        let mut sched = scheduler.borrow_mut();
        sched.evaluating += 1;
        scheduler.clone()
    };
    let mut stack = Vec::new();
    let result = catch_unwind(AssertUnwindSafe(|| evaluate(state, id, &mut stack, mode)));
    {
        let mut sched = scheduler.borrow_mut();
        sched.evaluating = sched.evaluating.saturating_sub(1);
    }
    match result {
        Ok(result) => {
            flush_if_idle(state);
            result
        }
        Err(panic) => resume_unwind(panic),
    }
}

fn evaluate<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    stack: &mut Vec<TargetNode>,
    mode: EvaluationMode,
) -> EvaluationResult<'scope, ()> {
    let target = {
        let state_ref = state
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        TargetNode {
            scope_id: state_ref.scope_id,
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
        if dep.scope_id == state.borrow().scope_id {
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
            let scheduler = state.borrow().scheduler.clone();
            let dep_scope = scheduler.borrow().get_scope(dep.scope_id);
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
            let max_dep_updated_epoch = state_ref
                .dependency_edges_of(id)
                .map(|(_, edge)| {
                    let dep = edge.target;
                    if dep.scope_id == state_ref.scope_id {
                        state_ref
                            .nodes
                            .get(dep.node)
                            .map(|target| target.updated_epoch)
                            .unwrap_or(0)
                    } else {
                        scheduler
                            .borrow()
                            .get_scope(dep.scope_id)
                            .map(|dep_scope| {
                                dep_scope
                                    .try_borrow()
                                    .ok()
                                    .map(|st| {
                                        st.nodes.get(dep.node).map(|n| n.updated_epoch).unwrap_or(0)
                                    })
                                    .unwrap_or(u64::MAX)
                            })
                            .unwrap_or(0)
                    }
                })
                .max()
                .unwrap_or(0);
            node.last_computed_epoch >= max_dep_updated_epoch
        } else {
            false
        }
    };
    if skip {
        let mut state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let current_epoch = state_ref.scheduler.borrow().current_epoch();
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
    storage: &NodeStorage<'scope>,
    scheduler: Rc<RefCell<GlobalScheduler>>,
) -> EvaluationResult<'scope, ComputationResult<'scope>> {
    let NodeStorage::Computation(computation) = storage else {
        return Err(EvaluationError::Runtime(ReactiveError::WrongKind));
    };

    let mut computation_lease = computation
        .computation
        .try_write(scheduler.clone())
        .map_err(EvaluationError::Runtime)?;
    let result = match &mut *computation_lease {
        Computation::Effect(callback) => {
            callback.call().map_err(EvaluationError::Callback)?;
            ComputationResult {
                produced_value: None,
                commit_value: false,
                notify: false,
                stop_after_run: false,
                initialize_watch: false,
            }
        }
        Computation::Previous(callback) => {
            let value_lease = computation
                .value
                .try_read(scheduler.clone())
                .map_err(EvaluationError::Runtime)?;
            let value = callback
                .compute((*value_lease).as_ref())
                .map_err(EvaluationError::Callback)?;
            drop(value_lease);
            ComputationResult {
                produced_value: Some(value),
                commit_value: true,
                notify: false,
                stop_after_run: false,
                initialize_watch: false,
            }
        }
        Computation::Watch(callback) => {
            let old_lease = computation
                .value
                .try_read(scheduler.clone())
                .map_err(EvaluationError::Runtime)?;
            let old = (*old_lease).as_ref();
            let new_value = callback.get().map_err(EvaluationError::Callback)?;
            let first_run = !callback.initialized();
            let changed = if first_run {
                true
            } else {
                old.is_none_or(|old| !new_value.try_eq(old))
            };
            let should_callback = if first_run {
                callback.immediate()
            } else {
                changed
            };
            if should_callback {
                let _observer_frame = ObserverFrame::push_untracked(scheduler.clone());
                callback
                    .call(&new_value, old)
                    .map_err(EvaluationError::Callback)?;
            }
            drop(old_lease);
            ComputationResult {
                produced_value: Some(new_value),
                commit_value: first_run || changed,
                notify: false,
                stop_after_run: should_callback && callback.once(),
                initialize_watch: first_run,
            }
        }
        Computation::Memo(callback) => {
            let old_lease = computation
                .value
                .try_read(scheduler.clone())
                .map_err(EvaluationError::Runtime)?;
            let old = (*old_lease).as_ref();
            let new_value = callback.compute(old).map_err(EvaluationError::Callback)?;
            drop(old_lease);
            let changed = {
                let old_lease = computation
                    .value
                    .try_read(scheduler)
                    .map_err(EvaluationError::Runtime)?;
                let old = (*old_lease).as_ref();
                old.is_none_or(|old| !new_value.try_eq(old))
            };
            ComputationResult {
                produced_value: Some(new_value),
                commit_value: changed,
                notify: changed,
                stop_after_run: false,
                initialize_watch: false,
            }
        }
        Computation::Derived(callback) => {
            let new_value = callback.compute().map_err(EvaluationError::Callback)?;
            ComputationResult {
                produced_value: Some(new_value),
                commit_value: true,
                notify: true,
                stop_after_run: false,
                initialize_watch: false,
            }
        }
    };
    drop(computation_lease);
    Ok(result)
}

fn commit_computation_value<'scope>(
    storage: &NodeStorage<'scope>,
    scheduler: Rc<RefCell<GlobalScheduler>>,
    value: AnyValue<'scope>,
    initialize_watch: bool,
) -> ReactiveResult<()> {
    let NodeStorage::Computation(computation) = storage else {
        return Err(ReactiveError::WrongKind);
    };
    if initialize_watch {
        let mut computation_lease = computation.computation.try_write(scheduler.clone())?;
        let Computation::Watch(watch) = &mut *computation_lease else {
            return Err(ReactiveError::WrongKind);
        };
        let mut value_lease = computation.value.try_write(scheduler.clone())?;
        let previous = (*value_lease).replace(value);
        watch.mark_initialized();
        drop(value_lease);
        drop(computation_lease);
        drop(previous);
    } else {
        let mut value_lease = computation.value.try_write(scheduler)?;
        let previous = (*value_lease).replace(value);
        drop(value_lease);
        drop(previous);
    }
    Ok(())
}

fn remember_panic(first: &mut Option<PanicData>, panic: PanicData) {
    if first.is_none() {
        *first = Some(panic);
    }
}

fn drop_value<'scope>(
    scheduler: Rc<RefCell<GlobalScheduler>>,
    value: Option<AnyValue<'scope>>,
) -> Option<PanicData> {
    let _observer_frame = ObserverFrame::push_untracked(scheduler);
    catch_unwind(AssertUnwindSafe(|| drop(value))).err()
}

fn drop_storage<'scope>(
    scheduler: Rc<RefCell<GlobalScheduler>>,
    storage: Rc<NodeStorage<'scope>>,
) -> Option<PanicData> {
    let _observer_frame = ObserverFrame::push_untracked(scheduler);
    catch_unwind(AssertUnwindSafe(|| drop(storage))).err()
}

fn run_node<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    mode: EvaluationMode,
) -> EvaluationResult<'scope, bool> {
    let (storage, first_child, cleanups, previous_owner, scheduler, scope_id) = {
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
        let scope_id = state_ref.scope_id;
        state_ref.begin_dependency_transaction(id);
        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.running = true;
            node.first_child = RawId::DANGLING;
        }
        state_ref.current_owner = Some(id);
        (
            storage,
            first_child,
            cleanups,
            previous_owner,
            scheduler,
            scope_id,
        )
    };

    let children_to_dispose: Vec<RawId> = state
        .try_borrow()
        .map_err(|_| EvaluationError::Runtime(ReactiveError::BorrowConflict))?
        .children_of_head(first_child)
        .collect();
    let mut execution_started = false;
    let mut observer_frame = None;
    let mut cleanup_errors = Vec::new();
    let outcome = catch_unwind(AssertUnwindSafe(
        || -> EvaluationResult<'scope, ComputationResult<'scope>> {
            let child_dispose = catch_unwind(AssertUnwindSafe(|| {
                dispose_nodes_collect(state, children_to_dispose)
            }));
            let mut cleanup_panic = match child_dispose {
                Ok(child_outcome) => {
                    cleanup_errors.extend(child_outcome.errors);
                    child_outcome.panic
                }
                Err(panic) => Some(panic),
            };
            let cleanup_outcome = run_cleanups(scheduler.clone(), cleanups);
            cleanup_errors.extend(cleanup_outcome.errors);
            if cleanup_panic.is_none() {
                cleanup_panic = cleanup_outcome.panic;
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
                        produced_value: None,
                        commit_value: false,
                        notify: false,
                        stop_after_run: false,
                        initialize_watch: false,
                    });
                }
                let scheduler = state_ref.scheduler.clone();
                scheduler.borrow_mut().executing += 1;
                execution_started = true;
                state_ref.current_owner = Some(id);
                observer_frame = Some(ObserverFrame::push(
                    scheduler,
                    Some(Observer {
                        scope_id: state_ref.scope_id,
                        node: id,
                    }),
                ));
                if let Some(node) = state_ref.nodes.get_mut(id) {
                    node.state = NodeState::Clean;
                }
            }

            execute_computation(&storage, scheduler.clone())
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
                state_ref.node_exists(id) && state_ref.is_active() && state_ref.scope_id == scope_id
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
                if computation_result.commit_value
                    && let Some(value) = std::mem::take(&mut computation_result.produced_value)
                {
                    let commit = catch_unwind(AssertUnwindSafe(|| {
                        let _observer_frame = ObserverFrame::push_untracked(scheduler.clone());
                        commit_computation_value(
                            &storage,
                            scheduler.clone(),
                            value,
                            computation_result.initialize_watch,
                        )
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

    if let Some(computation_result) = result.as_mut() {
        let value = std::mem::take(&mut computation_result.produced_value);
        if let Some(panic) = drop_value(scheduler.clone(), value) {
            remember_panic(&mut panic_data, panic);
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
        let now_epoch = state_ref.scheduler.borrow().current_epoch();
        if execution_started {
            let mut scheduler = state_ref.scheduler.borrow_mut();
            scheduler.executing = scheduler.executing.saturating_sub(1);
        }
        state_ref.set_context(previous_owner);
        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.running = false;
            node.last_computed_epoch = now_epoch;
            if failed {
                node.state = NodeState::Dirty;
            } else if notify && committed {
                node.updated_epoch = now_epoch;
                node.version = node.version.wrapping_add(1);
                state_ref.queue_dependents(id);
            }
        }
        if failed {
            if let Some(node) = state_ref.nodes.get_mut(id) {
                let first_child = node.first_child;
                node.first_child = RawId::DANGLING;
                failed_children = state_ref.children_of_head(first_child).collect();
            }
            if let Some(data) = state_ref.data.get_mut(id) {
                failed_cleanups = mem::take(&mut data.cleanups);
            }
        }
    }

    if failed {
        let child_dispose = catch_unwind(AssertUnwindSafe(|| {
            dispose_nodes_collect(state, failed_children)
        }));
        match child_dispose {
            Ok(outcome) => {
                cleanup_errors.extend(outcome.errors);
                if let Some(panic) = outcome.panic {
                    remember_panic(&mut panic_data, panic);
                }
            }
            Err(panic) => remember_panic(&mut panic_data, panic),
        }
        let cleanup_outcome = run_cleanups(scheduler.clone(), failed_cleanups);
        cleanup_errors.extend(cleanup_outcome.errors);
        if let Some(panic) = cleanup_outcome.panic {
            remember_panic(&mut panic_data, panic);
        }
    }

    if let Some(panic) = drop_storage(scheduler.clone(), storage) {
        remember_panic(&mut panic_data, panic);
    }

    if let Some(panic) = dispatch_cleanup_errors(scheduler.clone(), cleanup_errors) {
        remember_panic(&mut panic_data, panic);
    }

    if stop_after_run && !failed && committed {
        let stop_result = catch_unwind(AssertUnwindSafe(|| {
            dispose_nodes(state, vec![id]);
        }));
        if let Err(panic) = stop_result {
            remember_panic(&mut panic_data, panic);
        }
    }
    if let Some(panic) = panic_data {
        resume_unwind(panic);
    }
    if let Some(error) = operation_error {
        match (mode, error) {
            (EvaluationMode::Deferred, EvaluationError::Callback(error)) => {
                error.dispatch(ErrorPhase::Deferred);
                flush_if_idle(state);
                return Ok(true);
            }
            (EvaluationMode::Read, EvaluationError::Callback(error)) => {
                let value = error
                    .dispatch(ErrorPhase::Read)
                    .expect("read callback error must retain its payload");
                return Err(EvaluationError::User(value));
            }
            (_, error) => return Err(error),
        }
    }
    flush_if_idle(state);
    Ok(true)
}

pub(crate) fn run_initial<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
) -> EvaluationResult<'scope, ()> {
    match run_node(state, id, EvaluationMode::Initial)? {
        true => Ok(()),
        false => Err(EvaluationError::Runtime(ReactiveError::NoSuchNode)),
    }
}

pub(crate) fn flush_if_idle<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>) {
    let scheduler = state.borrow().scheduler.clone();
    let should_flush = scheduler.borrow().should_flush();
    if should_flush {
        run_global_queue(&scheduler);
    }
}

pub(crate) fn run_global_queue(scheduler: &Rc<RefCell<GlobalScheduler>>) {
    let acquired = {
        let mut sched = scheduler.borrow_mut();
        if sched.running_queue {
            false
        } else {
            sched.running_queue = true;
            true
        }
    };
    if !acquired {
        return;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut iterations = 0;
        loop {
            let next_task = scheduler.borrow_mut().global_queue.pop_front();
            let Some(task) = next_task else {
                break;
            };
            if !scheduler.borrow().is_scope_active(task.scope_id) {
                continue;
            }
            iterations += 1;
            assert!(
                iterations <= MAX_QUEUE_ITERATIONS,
                "silex_reactivity: effect 队列超过 {MAX_QUEUE_ITERATIONS} 次仍未收敛"
            );
            let scope_state = scheduler.borrow().get_scope(task.scope_id);
            if let Some(scope_state) = scope_state {
                {
                    let mut state_ref = scope_state
                        .try_borrow_mut()
                        .expect("ScopeState borrow failed in run_global_queue");
                    if let Some(node) = state_ref.nodes.get_mut(task.node) {
                        node.queued = false;
                    }
                }
                if let Err(error) = evaluate_root(&scope_state, task.node, EvaluationMode::Deferred)
                {
                    panic!("silex_reactivity: effect queue evaluation failed: {error}");
                }
            }
        }
    }));

    if outcome.is_err() {
        recover_after_queue_error(scheduler);
    }
    scheduler.borrow_mut().running_queue = false;

    if let Err(panic) = outcome {
        resume_unwind(panic);
    }
}

fn recover_after_queue_error(scheduler: &Rc<RefCell<GlobalScheduler>>) {
    let scope_ids = scheduler.borrow().active_scope_ids();
    scheduler.borrow_mut().global_queue.clear();

    for scope_id in scope_ids {
        let Some(scope_state) = scheduler.borrow().get_scope(scope_id) else {
            continue;
        };
        let Ok(mut state) = scope_state.try_borrow_mut() else {
            continue;
        };
        for node in state.nodes.values_mut() {
            if node.state == NodeState::Check {
                node.state = NodeState::Dirty;
            }
            node.queued = false;
        }
    }
}
