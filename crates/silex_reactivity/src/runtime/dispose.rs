//! Node hierarchy disposal and scope cleanup execution.

use super::model::{EdgeId, ScopeState};
use crate::internal::{RawId, value::OnceThunk};
use std::{
    cell::RefCell,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

fn run_cleanups<'scope>(cleanups: Vec<OnceThunk<'scope>>) {
    let mut first_panic = None;
    for cleanup in cleanups {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| cleanup.call()))
            && first_panic.is_none()
        {
            first_panic = Some(panic);
        }
    }
    if let Some(panic) = first_panic {
        resume_unwind(panic);
    }
}

pub(crate) fn dispose_all<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>) {
    loop {
        let roots = state
            .try_borrow()
            .expect("ScopeState borrow failed during dispose_all")
            .roots
            .clone();
        if roots.is_empty() {
            break;
        }
        dispose_nodes(state, roots);
    }

    let cleanups = mem::take(
        &mut state
            .try_borrow_mut()
            .expect("ScopeState borrow_mut failed during dispose_all")
            .root_cleanups,
    );
    run_cleanups(cleanups);
}

enum DisposeStep<'scope> {
    Enter(RawId),
    Exit { cleanups: Vec<OnceThunk<'scope>> },
}

pub(crate) fn dispose_nodes<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, roots: Vec<RawId>) {
    let mut stack = Vec::with_capacity(roots.len());
    stack.extend(roots.into_iter().rev().map(DisposeStep::Enter));
    while let Some(step) = stack.pop() {
        match step {
            DisposeStep::Enter(id) => {
                let mut state_ref = state
                    .try_borrow_mut()
                    .expect("ScopeState borrow_mut failed during dispose_nodes");
                let (children, data) = if let Some(node) = state_ref.nodes.get(id).copied() {
                    state_ref.clear_dependencies(id);

                    let subscriber_edges: Vec<EdgeId> = state_ref
                        .subscriber_edges_of(id)
                        .map(|(edge_id, _)| edge_id)
                        .collect();
                    for edge_id in subscriber_edges {
                        state_ref.edges.remove(edge_id);
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
                let cleanups = data.map(|d| d.cleanups).unwrap_or_default();
                stack.push(DisposeStep::Exit { cleanups });
                stack.extend(children.into_iter().rev().map(DisposeStep::Enter));
            }
            DisposeStep::Exit { cleanups } => {
                run_cleanups(cleanups);
            }
        }
    }
}
