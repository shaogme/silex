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

    if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(storage))) {
        remember_panic(&mut outcome.panic, panic);
    }
    outcome
}

pub(crate) fn dispose_all<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>) {
    let scheduler = state
        .try_borrow()
        .expect("ScopeState borrow failed during dispose_all")
        .scheduler
        .clone();
    let mut first_panic = None;
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

    loop {
        let cleanups = mem::take(
            &mut state
                .try_borrow_mut()
                .expect("ScopeState borrow_mut failed during dispose_all")
                .root_cleanups,
        );
        if cleanups.is_empty() {
            break;
        }
        let outcome = run_cleanups(scheduler.clone(), cleanups);
        if let Some(panic) = outcome.panic {
            remember_panic(&mut first_panic, panic);
        }
        if let Some(panic) = dispatch_cleanup_errors(scheduler.clone(), outcome.errors) {
            remember_panic(&mut first_panic, panic);
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

pub(crate) fn dispose_nodes<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, roots: Vec<RawId>) {
    let scheduler = state
        .try_borrow()
        .expect("ScopeState borrow failed during dispose_nodes")
        .scheduler
        .clone();
    let (mut first_panic, cleanup_errors) = {
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
                            let observer_state = scheduler.borrow().get_scope(subscriber.scope_id);
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
                            } else if let Some(observer_state) =
                                scheduler.borrow().get_scope(subscriber.scope_id)
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
        (first_panic, cleanup_errors)
    };

    if let Some(panic) = dispatch_cleanup_errors(scheduler, cleanup_errors) {
        remember_panic(&mut first_panic, panic);
    }
    if let Some(panic) = first_panic {
        resume_unwind(panic);
    }
}
