//! Node hierarchy disposal and scope cleanup execution.

use super::{
    model::{DisposalScratch, NodeData, ScopeState},
    scheduler::{GlobalScheduler, ObserverFrame, OwnerId, TargetNode},
    storage::CleanupThunk,
};
use crate::{
    ReactiveError,
    borrow::SharedCell,
    error::{ErrorEvent, ErrorPhase, HandlerError},
    internal::NodeId,
};
use std::{
    any::Any,
    collections::HashSet,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

pub(crate) type PanicData = Box<dyn Any + Send>;

/// Results collected while disposal continues after an individual callback
/// or payload drop fails.
pub(crate) struct CleanupOutcome<'scope> {
    pub(crate) errors: Vec<ErrorEvent<'scope>>,
    pub(crate) panics: Vec<PanicData>,
    pub(crate) handler_errors: Vec<HandlerError>,
    pub(crate) runtime_errors: Vec<ReactiveError>,
}

impl<'scope> CleanupOutcome<'scope> {
    fn new() -> Self {
        Self {
            errors: Vec::new(),
            panics: Vec::new(),
            handler_errors: Vec::new(),
            runtime_errors: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.errors.append(&mut other.errors);
        self.panics.append(&mut other.panics);
        self.handler_errors.append(&mut other.handler_errors);
        self.runtime_errors.append(&mut other.runtime_errors);
    }
}

pub(crate) fn run_cleanups<'scope>(
    scheduler: SharedCell<GlobalScheduler>,
    cleanups: Vec<CleanupThunk<'scope>>,
) -> CleanupOutcome<'scope> {
    let mut outcome = CleanupOutcome::new();
    let _observer_frame = match ObserverFrame::push_untracked(scheduler) {
        Ok(frame) => frame,
        Err(error) => {
            outcome.runtime_errors.push(error);
            return outcome;
        }
    };
    for cleanup in cleanups {
        match catch_unwind(AssertUnwindSafe(|| cleanup.call())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => outcome.errors.push(error),
            Err(panic) => outcome.panics.push(panic),
        }
    }
    outcome
}

pub(crate) fn dispatch_cleanup_errors<'scope>(
    scheduler: SharedCell<GlobalScheduler>,
    errors: Vec<ErrorEvent<'scope>>,
) -> CleanupOutcome<'scope> {
    let mut outcome = CleanupOutcome::new();
    if errors.is_empty() {
        return outcome;
    }
    let _observer_frame = match ObserverFrame::push_untracked(scheduler) {
        Ok(frame) => frame,
        Err(error) => {
            outcome.runtime_errors.push(error);
            return outcome;
        }
    };
    for error in errors {
        match catch_unwind(AssertUnwindSafe(|| error.dispatch(ErrorPhase::Deferred))) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => outcome.handler_errors.push(error),
            Err(panic) => outcome.panics.push(panic),
        }
    }
    outcome
}

fn drop_node_data<'scope>(
    scheduler: SharedCell<GlobalScheduler>,
    data: NodeData<'scope>,
) -> CleanupOutcome<'scope> {
    let NodeData { storage, cleanups } = data;
    let mut outcome = run_cleanups(scheduler.clone(), cleanups);
    let _observer_frame = match ObserverFrame::push_untracked(scheduler) {
        Ok(frame) => frame,
        Err(error) => {
            outcome.runtime_errors.push(error);
            return outcome;
        }
    };
    if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(storage))) {
        outcome.panics.push(panic);
    }
    outcome
}

pub(crate) struct FinalCleanupPlan<'scope> {
    node_batches: Vec<Vec<CleanupThunk<'scope>>>,
    root_cleanups: Vec<CleanupThunk<'scope>>,
}

enum CleanupPlanStep {
    Enter(NodeId),
    Exit(NodeId),
}

fn collect_final_cleanup_plan<'scope>(
    state: &ScopeState<'scope>,
) -> Result<FinalCleanupPlan<'scope>, ReactiveError> {
    let roots = state.try_borrow()?.roots.to_vec();
    let mut stack = Vec::with_capacity(roots.len());
    stack.extend(roots.into_iter().rev().map(CleanupPlanStep::Enter));
    let mut node_batches = Vec::new();

    while let Some(step) = stack.pop() {
        match step {
            CleanupPlanStep::Enter(id) => {
                let children = {
                    let state_ref = state.try_borrow()?;
                    let Some(node) = state_ref.nodes.get(id).copied() else {
                        continue;
                    };
                    state_ref
                        .children_of_head(node.first_child)
                        .collect::<Vec<_>>()
                };
                stack.push(CleanupPlanStep::Exit(id));
                stack.extend(children.into_iter().rev().map(CleanupPlanStep::Enter));
            }
            CleanupPlanStep::Exit(id) => {
                let cleanups = state
                    .try_borrow_mut()?
                    .data
                    .get_mut(id)
                    .map(|data| mem::take(&mut data.cleanups))
                    .unwrap_or_default();
                if !cleanups.is_empty() {
                    node_batches.push(cleanups);
                }
            }
        }
    }

    let root_cleanups = mem::take(&mut state.try_borrow_mut()?.root_cleanups);
    Ok(FinalCleanupPlan {
        node_batches,
        root_cleanups,
    })
}

fn run_final_cleanup_plan<'scope>(
    scheduler: SharedCell<GlobalScheduler>,
    plan: FinalCleanupPlan<'scope>,
) -> CleanupOutcome<'scope> {
    let mut outcome = CleanupOutcome::new();
    let mut node_errors = Vec::new();
    for cleanups in plan.node_batches {
        let mut node_outcome = run_cleanups(scheduler.clone(), cleanups);
        node_errors.append(&mut node_outcome.errors);
        outcome.append(node_outcome);
    }
    outcome.append(dispatch_cleanup_errors(scheduler.clone(), node_errors));

    let mut root_outcome = run_cleanups(scheduler.clone(), plan.root_cleanups);
    let root_errors = mem::take(&mut root_outcome.errors);
    outcome.append(root_outcome);
    outcome.append(dispatch_cleanup_errors(scheduler, root_errors));
    outcome
}

pub(crate) fn dispose_all<'scope>(state: &ScopeState<'scope>) -> CleanupOutcome<'scope> {
    let mut outcome = CleanupOutcome::new();
    let scheduler = match state.try_borrow() {
        Ok(state_ref) => state_ref.scheduler.clone(),
        Err(error) => {
            outcome.runtime_errors.push(error);
            return outcome;
        }
    };

    let cleanup_plan = match state.try_borrow_mut() {
        Ok(mut state_ref) => state_ref.begin_cleanup().err(),
        Err(error) => Some(error),
    };
    if let Some(error) = cleanup_plan {
        outcome.runtime_errors.push(error);
        return outcome;
    }

    let plan = match collect_final_cleanup_plan(state) {
        Ok(plan) => plan,
        Err(error) => {
            outcome.runtime_errors.push(error);
            return outcome;
        }
    };
    outcome.append(run_final_cleanup_plan(scheduler.clone(), plan));

    if let Err(error) = state
        .try_borrow_mut()
        .and_then(|mut state| state.begin_detaching())
    {
        outcome.runtime_errors.push(error);
        return outcome;
    }

    loop {
        let roots = match state.try_borrow() {
            Ok(state_ref) => state_ref.roots.to_vec(),
            Err(error) => {
                outcome.runtime_errors.push(error);
                return outcome;
            }
        };
        if roots.is_empty() {
            break;
        }
        match dispose_nodes_collect(state, roots) {
            Ok(mut node_outcome) => {
                let errors = mem::take(&mut node_outcome.errors);
                outcome.append(node_outcome);
                outcome.append(dispatch_cleanup_errors(scheduler.clone(), errors));
            }
            Err(error) => {
                outcome.runtime_errors.push(error);
                return outcome;
            }
        }
    }

    if let Err(error) = state
        .try_borrow_mut()
        .and_then(|mut state| state.finish_dispose())
    {
        outcome.runtime_errors.push(error);
    }
    outcome
}

enum DisposeStep<'scope> {
    Enter(NodeId),
    Exit(Option<NodeData<'scope>>),
}

fn collect_disposal_nodes<'scope>(
    state: &ScopeState<'scope>,
    roots: &[NodeId],
    scratch: &mut DisposalScratch,
) -> Result<(), ReactiveError> {
    let state_ref = state.try_borrow()?;
    scratch.pending.extend(roots.iter().copied());
    while let Some(id) = scratch.pending.pop() {
        if !scratch.visited.insert(id) {
            continue;
        }
        let Some(node) = state_ref.nodes.get(id) else {
            continue;
        };
        scratch.nodes.push(id);
        scratch
            .pending
            .extend(state_ref.children_of_head(node.first_child));
    }
    Ok(())
}

fn preflight_node_disposal<'scope>(
    state: &ScopeState<'scope>,
    nodes: &[NodeId],
    external_owner_ids: &mut HashSet<OwnerId>,
) -> Result<(), ReactiveError> {
    let (owner_id, scheduler) = {
        let state_ref = state.try_borrow()?;
        for id in nodes {
            let Some(_) = state_ref.nodes.get(*id) else {
                continue;
            };
            for target in state_ref
                .dependency_edges_of(*id)
                .chain(state_ref.subscriber_edges_of(*id))
            {
                if target.owner_id != state_ref.owner_id {
                    external_owner_ids.insert(target.owner_id);
                }
            }
        }
        (state_ref.owner_id, state_ref.scheduler.clone())
    };

    scheduler
        .try_borrow_mut()
        .map_err(|_| ReactiveError::BorrowConflict)?;
    for external_owner_id in external_owner_ids.iter().copied() {
        if external_owner_id == owner_id {
            continue;
        }
        if let Some(scope) = scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .get_scope_for_edge_cleanup(external_owner_id)?
        {
            scope
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?;
        }
    }
    state.try_borrow_mut()?;
    Ok(())
}

pub(crate) fn dispose_nodes_collect<'scope>(
    state: &ScopeState<'scope>,
    roots: Vec<NodeId>,
) -> Result<CleanupOutcome<'scope>, ReactiveError> {
    let scheduler = state.try_borrow()?.scheduler.clone();
    let mut scratch = {
        let mut state_ref = state.try_borrow_mut()?;
        if let Some(scratch) = state_ref.disposal_scratch_pool.pop() {
            #[cfg(feature = "test-support")]
            {
                state_ref.scratch_stats.disposal_pool_hits =
                    state_ref.scratch_stats.disposal_pool_hits.saturating_add(1);
            }
            scratch
        } else {
            #[cfg(feature = "test-support")]
            {
                state_ref.scratch_stats.disposal_pool_misses = state_ref
                    .scratch_stats
                    .disposal_pool_misses
                    .saturating_add(1);
            }
            DisposalScratch::default()
        }
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        dispose_nodes_collect_with_scratch(state, roots, scheduler, &mut scratch)
    }));
    #[cfg(feature = "test-support")]
    state.try_borrow_mut()?.record_disposal_scratch(&scratch);
    scratch.reset();
    state.try_borrow_mut()?.disposal_scratch_pool.push(scratch);
    match result {
        Ok(result) => result,
        Err(panic) => resume_unwind(panic),
    }
}

fn dispose_nodes_collect_with_scratch<'scope>(
    state: &ScopeState<'scope>,
    roots: Vec<NodeId>,
    scheduler: SharedCell<GlobalScheduler>,
    scratch: &mut DisposalScratch,
) -> Result<CleanupOutcome<'scope>, ReactiveError> {
    collect_disposal_nodes(state, &roots, scratch)?;
    preflight_node_disposal(state, &scratch.nodes, &mut scratch.external_owner_ids)?;
    let _observer_frame = ObserverFrame::push_untracked(scheduler.clone())?;
    let mut outcome = CleanupOutcome::new();
    let mut stack = Vec::with_capacity(roots.len());
    stack.extend(roots.into_iter().rev().map(DisposeStep::Enter));

    while let Some(step) = stack.pop() {
        match step {
            DisposeStep::Enter(id) => {
                let (children, data) = {
                    let mut state_ref = state.try_borrow_mut()?;
                    let Some(node) = state_ref.nodes.get(id).copied() else {
                        continue;
                    };
                    let source_target = TargetNode {
                        owner_id: state_ref.owner_id,
                        node: id,
                    };
                    state_ref.take_subscribers_into(id, &mut scratch.removed_targets);
                    let scheduler = state_ref.scheduler.clone();
                    scheduler
                        .try_borrow_mut()
                        .map_err(|_| ReactiveError::BorrowConflict)?
                        .cancel_effect(source_target);

                    state_ref.clear_dependencies(id)?;
                    for subscriber in scratch.removed_targets.iter().copied() {
                        if subscriber.owner_id == state_ref.owner_id {
                            state_ref.remove_dependency(subscriber.node, source_target);
                        } else if let Some(observer_state) = scheduler
                            .try_borrow()
                            .map_err(|_| ReactiveError::BorrowConflict)?
                            .get_scope_for_edge_cleanup(subscriber.owner_id)?
                        {
                            observer_state
                                .try_borrow_mut()
                                .map_err(|_| ReactiveError::BorrowConflict)?
                                .remove_dependency(subscriber.node, source_target);
                        }
                    }

                    state_ref
                        .dependency_transactions
                        .retain(|transaction| transaction.observer != id);
                    let children: Vec<NodeId> =
                        state_ref.children_of_head(node.first_child).collect();
                    let data = state_ref.data.remove(id);
                    if state_ref.current_owner == Some(id) {
                        state_ref.current_owner = None;
                    }
                    state_ref.unlink_child(node.parent, id);
                    state_ref.adjacency.remove(id);
                    state_ref.nodes.remove(id);
                    (children, data)
                };
                stack.push(DisposeStep::Exit(data));
                stack.extend(children.into_iter().rev().map(DisposeStep::Enter));
            }
            DisposeStep::Exit(data) => {
                if let Some(data) = data {
                    outcome.append(drop_node_data(scheduler.clone(), data));
                }
            }
        }
    }
    Ok(outcome)
}

pub(crate) fn dispose_nodes<'scope>(
    state: &ScopeState<'scope>,
    roots: Vec<NodeId>,
) -> Result<CleanupOutcome<'scope>, ReactiveError> {
    let scheduler = state.try_borrow()?.scheduler.clone();
    let mut outcome = dispose_nodes_collect(state, roots)?;
    let errors = mem::take(&mut outcome.errors);
    outcome.append(dispatch_cleanup_errors(scheduler, errors));
    Ok(outcome)
}
