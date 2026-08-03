//! Node hierarchy disposal and scope cleanup execution.

use super::{
    model::{EdgeId, NodeData, ScopeState},
    scheduler::TargetNode,
};
use crate::internal::{RawId, value::OnceThunk};
use std::{
    any::Any,
    cell::RefCell,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

type PanicPayload = Box<dyn Any + Send>;

fn remember_panic(first_panic: &mut Option<PanicPayload>, panic: PanicPayload) {
    if first_panic.is_none() {
        *first_panic = Some(panic);
    }
}

pub(crate) fn run_cleanups<'scope>(cleanups: Vec<OnceThunk<'scope>>) -> Option<PanicPayload> {
    let mut first_panic = None;
    for cleanup in cleanups {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| cleanup.call())) {
            remember_panic(&mut first_panic, panic);
        }
    }
    first_panic
}

fn drop_node_data<'scope>(data: NodeData<'scope>) -> Option<PanicPayload> {
    let NodeData {
        value,
        cleanups,
        payload,
        computation,
    } = data;
    let mut first_panic = run_cleanups(cleanups);

    for value in [value] {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(value))) {
            remember_panic(&mut first_panic, panic);
        }
    }
    for payload in [payload] {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            remember_panic(&mut first_panic, panic);
        }
    }
    for computation in [computation] {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(computation))) {
            remember_panic(&mut first_panic, panic);
        }
    }
    first_panic
}

pub(crate) fn dispose_all<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>) {
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
        if let Some(panic) = run_cleanups(cleanups) {
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
    let mut first_panic = None;
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

                    let children: Vec<RawId> =
                        state_ref.children_of_head(node.first_child).collect();
                    let data = state_ref.data.remove(id);
                    state_ref.nodes.remove(id);
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
                if let Some(data) = data
                    && let Some(panic) = drop_node_data(data)
                {
                    remember_panic(&mut first_panic, panic);
                }
            }
        }
    }

    if let Some(panic) = first_panic {
        resume_unwind(panic);
    }
}
