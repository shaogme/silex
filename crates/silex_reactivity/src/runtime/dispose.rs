//! Node hierarchy disposal and scope cleanup execution.

use super::{
    model::{EdgeId, NodeData, ScopeState},
    scheduler::{GlobalScheduler, ObserverFrame, TargetNode},
};
use crate::{
    error::{ErrorEvent, ErrorPhase},
    internal::{RawId, value::CleanupThunk},
};
use std::{
    any::Any,
    cell::RefCell,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

type PanicData = Box<dyn Any + Send>;

pub(crate) struct CleanupOutcome<'scope> {
    pub(crate) errors: Vec<ErrorEvent<'scope>>,
    pub(crate) panic: Option<PanicData>,
}

fn remember_panic(first_panic: &mut Option<PanicData>, panic: PanicData) {
    if first_panic.is_none() {
        *first_panic = Some(panic);
    }
}

pub(crate) fn run_cleanups<'scope>(
    scheduler: Rc<RefCell<GlobalScheduler>>,
    cleanups: Vec<CleanupThunk<'scope>>,
) -> CleanupOutcome<'scope> {
    let _observer_frame = ObserverFrame::push_untracked(scheduler);
    let mut first_panic = None;
    let mut errors = Vec::new();
    for cleanup in cleanups {
        match catch_unwind(AssertUnwindSafe(|| cleanup.call())) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => errors.push(error),
            Err(panic) => remember_panic(&mut first_panic, panic),
        }
    }
    CleanupOutcome {
        errors,
        panic: first_panic,
    }
}

pub(crate) fn dispatch_cleanup_errors<'scope>(
    scheduler: Rc<RefCell<GlobalScheduler>>,
    errors: Vec<ErrorEvent<'scope>>,
) -> Option<PanicData> {
    if errors.is_empty() {
        return None;
    }
    let mut first_panic = None;
    {
        let _observer_frame = ObserverFrame::push_untracked(scheduler);
        for error in errors {
            if let Err(panic) = catch_unwind(AssertUnwindSafe(|| {
                error.dispatch(ErrorPhase::Deferred);
            })) {
                remember_panic(&mut first_panic, panic);
            }
        }
    }
    first_panic
}

fn drop_node_data<'scope>(
    scheduler: Rc<RefCell<GlobalScheduler>>,
    data: NodeData<'scope>,
) -> CleanupOutcome<'scope> {
    let NodeData { storage, cleanups } = data;
    let mut outcome = run_cleanups(scheduler.clone(), cleanups);

    let _observer_frame = ObserverFrame::push_untracked(scheduler);
    if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(storage))) {
        remember_panic(&mut outcome.panic, panic);
    }
    outcome
}

pub(crate) struct FinalCleanupPlan<'scope> {
    node_batches: Vec<Vec<CleanupThunk<'scope>>>,
    root_cleanups: Vec<CleanupThunk<'scope>>,
}

enum CleanupPlanStep {
    Enter(RawId),
    Exit(RawId),
}

fn collect_final_cleanup_plan<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
) -> FinalCleanupPlan<'scope> {
    let roots = state
        .try_borrow()
        .expect("ScopeState borrow failed during cleanup plan collection")
        .roots
        .clone();
    let mut stack = Vec::with_capacity(roots.len());
    stack.extend(roots.into_iter().rev().map(CleanupPlanStep::Enter));
    let mut node_batches = Vec::new();

    while let Some(step) = stack.pop() {
        match step {
            CleanupPlanStep::Enter(id) => {
                let children = {
                    let state_ref = state
                        .try_borrow()
                        .expect("ScopeState borrow failed during cleanup plan collection");
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
                    .try_borrow_mut()
                    .expect("ScopeState borrow_mut failed during cleanup plan collection")
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

    let root_cleanups = mem::take(
        &mut state
            .try_borrow_mut()
            .expect("ScopeState borrow_mut failed during cleanup plan collection")
            .root_cleanups,
    );
    FinalCleanupPlan {
        node_batches,
        root_cleanups,
    }
}

fn run_final_cleanup_plan<'scope>(
    scheduler: Rc<RefCell<GlobalScheduler>>,
    plan: FinalCleanupPlan<'scope>,
) -> Option<PanicData> {
    let mut first_panic = None;
    let mut node_errors = Vec::new();
    for cleanups in plan.node_batches {
        let outcome = run_cleanups(scheduler.clone(), cleanups);
        node_errors.extend(outcome.errors);
        if let Some(panic) = outcome.panic {
            remember_panic(&mut first_panic, panic);
        }
    }
    if let Some(panic) = dispatch_cleanup_errors(scheduler.clone(), node_errors) {
        remember_panic(&mut first_panic, panic);
    }

    let root_outcome = run_cleanups(scheduler.clone(), plan.root_cleanups);
    if let Some(panic) = root_outcome.panic {
        remember_panic(&mut first_panic, panic);
    }
    if let Some(panic) = dispatch_cleanup_errors(scheduler, root_outcome.errors) {
        remember_panic(&mut first_panic, panic);
    }
    first_panic
}

pub(crate) fn dispose_all<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>) {
    let scheduler = state
        .try_borrow()
        .expect("ScopeState borrow failed during dispose_all")
        .scheduler
        .clone();
    let plan = collect_final_cleanup_plan(state);
    let mut first_panic = run_final_cleanup_plan(scheduler.clone(), plan);

    state
        .try_borrow_mut()
        .expect("ScopeState borrow_mut failed during dispose_all")
        .begin_node_disposal();

    loop {
        let roots = state
            .try_borrow()
            .expect("ScopeState borrow failed during dispose_all")
            .roots
            .clone();
        if roots.is_empty() {
            break;
        }
        let result = catch_unwind(AssertUnwindSafe(|| dispose_nodes(state, roots)));
        if let Err(panic) = result {
            remember_panic(&mut first_panic, panic);
            break;
        }
    }

    if let Some(panic) = first_panic {
        resume_unwind(panic);
    }
}

enum DisposeStep<'scope> {
    Enter(RawId),
    Exit { data: Option<NodeData<'scope>> },
}

pub(crate) fn dispose_nodes_collect<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    roots: Vec<RawId>,
) -> CleanupOutcome<'scope> {
    let scheduler = state
        .try_borrow()
        .expect("ScopeState borrow failed during dispose_nodes")
        .scheduler
        .clone();
    {
        let _observer_frame = ObserverFrame::push_untracked(scheduler.clone());
        let mut first_panic = None;
        let mut cleanup_errors = Vec::new();
        let mut stack = Vec::with_capacity(roots.len());
        stack.extend(roots.into_iter().rev().map(DisposeStep::Enter));
        while let Some(step) = stack.pop() {
            match step {
                DisposeStep::Enter(id) => {
                    let mut state_ref = state
                        .try_borrow_mut()
                        .expect("ScopeState borrow_mut failed during dispose_nodes");
                    let (children, data) = if let Some(node) = state_ref.nodes.get(id).copied() {
                        let source_target = TargetNode {
                            scope_id: state_ref.scope_id,
                            node: id,
                        };
                        let subscriber_edges: Vec<(EdgeId, TargetNode)> = state_ref
                            .subscriber_edges_of(id)
                            .map(|(edge_id, edge)| (edge_id, edge.target))
                            .collect();
                        let scheduler = state_ref.scheduler.clone();
                        scheduler.borrow_mut().cancel_effect(source_target);
                        for (_, subscriber) in &subscriber_edges {
                            if subscriber.scope_id == state_ref.scope_id {
                                continue;
                            }
                            let observer_state = scheduler
                                .borrow()
                                .get_scope_for_edge_cleanup(subscriber.scope_id);
                            if let Some(observer_state) = observer_state {
                                observer_state
                                    .try_borrow_mut()
                                    .expect("observer scope borrow failed during dispose_nodes");
                            }
                        }

                        state_ref.clear_dependencies(id);

                        for (edge_id, subscriber) in subscriber_edges {
                            state_ref.edges.remove(edge_id);
                            if subscriber.scope_id == state_ref.scope_id {
                                state_ref.remove_dependency(subscriber.node, source_target);
                            } else if let Some(observer_state) = scheduler
                                .borrow()
                                .get_scope_for_edge_cleanup(subscriber.scope_id)
                            {
                                observer_state
                                    .try_borrow_mut()
                                    .expect("observer scope borrow failed during dispose_nodes")
                                    .remove_dependency(subscriber.node, source_target);
                            }
                        }

                        state_ref
                            .dependency_transactions
                            .retain(|transaction| transaction.observer != id);
                        let children: Vec<RawId> =
                            state_ref.children_of_head(node.first_child).collect();
                        let data = state_ref.data.remove(id);
                        state_ref.nodes.remove(id);
                        if state_ref.current_owner == Some(id) {
                            state_ref.current_owner = None;
                        }
                        state_ref.unlink_child(node.parent, id, node.next_sibling);
                        (children, data)
                    } else {
                        (Vec::new(), None)
                    };
                    drop(state_ref);
                    stack.push(DisposeStep::Exit { data });
                    stack.extend(children.into_iter().rev().map(DisposeStep::Enter));
                }
                DisposeStep::Exit { data } => {
                    if let Some(data) = data {
                        let outcome = drop_node_data(scheduler.clone(), data);
                        cleanup_errors.extend(outcome.errors);
                        if let Some(panic) = outcome.panic {
                            remember_panic(&mut first_panic, panic);
                        }
                    }
                }
            }
        }
        CleanupOutcome {
            errors: cleanup_errors,
            panic: first_panic,
        }
    }
}

pub(crate) fn dispose_nodes<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, roots: Vec<RawId>) {
    let scheduler = state
        .try_borrow()
        .expect("ScopeState borrow failed during dispose_nodes")
        .scheduler
        .clone();
    let outcome = dispose_nodes_collect(state, roots);
    let mut first_panic = outcome.panic;
    if let Some(panic) = dispatch_cleanup_errors(scheduler, outcome.errors) {
        remember_panic(&mut first_panic, panic);
    }
    if let Some(panic) = first_panic {
        resume_unwind(panic);
    }
}
