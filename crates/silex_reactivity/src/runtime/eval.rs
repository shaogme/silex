//! Computation evaluation engine and queue flush scheduler.

use super::{
    dispose::dispose_nodes,
    model::{NodeState, ScopeState},
    scheduler::{GlobalScheduler, Observer, TargetNode},
};
use crate::{
    ReactiveError, ReactiveResult,
    handle::NodeKindTag,
    internal::{RawId, value::Computation},
};
use std::{
    cell::RefCell,
    mem,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

const MAX_QUEUE_ITERATIONS: usize = 100_000;

pub(crate) fn prepare_read<'scope>(
    state: &Rc<RefCell<ScopeState<'scope>>>,
    id: RawId,
    track: bool,
) -> ReactiveResult<()> {
    let settled = state
        .try_borrow()
        .map_err(|_| ReactiveError::Reentrant)?
        .is_settled(id);
    if !settled {
        evaluate_root(state, id)?;
    }
    if track {
        state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::Reentrant)?
            .track(id);
    }
    flush_if_idle(state);
    Ok(())
}

fn evaluate_root<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) -> ReactiveResult<()> {
    let scheduler = {
        let state_ref = state
            .try_borrow_mut()
            .map_err(|_| ReactiveError::Reentrant)?;
        let scheduler = state_ref.scheduler.clone();
        let mut sched = scheduler.borrow_mut();
        sched.evaluating += 1;
        scheduler.clone()
    };
    let mut stack = Vec::new();
    let result = catch_unwind(AssertUnwindSafe(|| evaluate(state, id, &mut stack)));
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
) -> ReactiveResult<()> {
    let target = {
        let state_ref = state.try_borrow().map_err(|_| ReactiveError::Reentrant)?;
        TargetNode {
            scope_id: state_ref.scope_id,
            node: id,
        }
    };
    let (node_state, running, dependencies) = {
        let state_ref = state.try_borrow().map_err(|_| ReactiveError::Reentrant)?;
        let node = state_ref.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if node.state == NodeState::Clean || node.running {
            return Ok(());
        }
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
        return Err(ReactiveError::Reentrant);
    }
    stack.push(target);
    for dep in &dependencies {
        if dep.scope_id == state.borrow().scope_id {
            let dependency_state = state
                .try_borrow()
                .map_err(|_| ReactiveError::Reentrant)?
                .nodes
                .get(dep.node)
                .map(|node| node.state);
            if dependency_state.is_some_and(|state| state != NodeState::Clean) {
                evaluate(state, dep.node, stack)?;
            }
        } else {
            let scheduler = state.borrow().scheduler.clone();
            let dep_scope = scheduler.borrow().get_scope(dep.scope_id);
            if let Some(dep_scope) = dep_scope {
                let dependency_state = dep_scope
                    .try_borrow()
                    .map_err(|_| ReactiveError::Reentrant)?
                    .nodes
                    .get(dep.node)
                    .map(|node| node.state);
                if dependency_state.is_some_and(|state| state != NodeState::Clean) {
                    evaluate(&dep_scope, dep.node, stack)?;
                }
            }
        }
    }
    stack.pop();

    let skip = {
        let state_ref = state.try_borrow().map_err(|_| ReactiveError::Reentrant)?;
        let Some(node) = state_ref.nodes.get(id) else {
            return Err(ReactiveError::NoSuchNode);
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
            .expect("ScopeState borrow failed during skip epoch update");
        let current_epoch = state_ref.scheduler.borrow().current_epoch();
        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.state = NodeState::Clean;
            node.last_computed_epoch = current_epoch;
        }
        return Ok(());
    }
    run_node(state, id)
        .then_some(())
        .ok_or(ReactiveError::NoSuchNode)
}

fn run_node<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) -> bool {
    let (computation, mut old, first_child, cleanups, previous) = {
        let mut state_ref = state
            .try_borrow_mut()
            .expect("ScopeState borrow failed at start of run_node");
        let (is_computation, is_running, is_memo_or_derived, first_child) = {
            let Some(node) = state_ref.nodes.get(id) else {
                return false;
            };
            let is_memo_or_derived = matches!(node.kind, NodeKindTag::Memo | NodeKindTag::Derived);
            (
                node.is_computation(),
                node.running,
                is_memo_or_derived,
                node.first_child,
            )
        };

        if !is_computation || is_running {
            return false;
        }

        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.running = true;
            node.first_child = RawId::DANGLING;
        }

        let old = is_memo_or_derived
            .then(|| {
                state_ref
                    .data
                    .get_mut(id)
                    .and_then(|data| data.value.take())
            })
            .flatten();

        let cleanups = state_ref
            .data
            .get_mut(id)
            .map(|data| mem::take(&mut data.cleanups))
            .unwrap_or_default();

        let computation = state_ref
            .data
            .get_mut(id)
            .and_then(|data| data.computation.take());

        let prev_owner = state_ref.current_owner;
        let prev_obs = state_ref.scheduler.borrow().observer();
        state_ref.clear_dependencies(id);
        state_ref.current_owner = prev_owner;
        state_ref.scheduler.borrow_mut().set_observer(prev_obs);

        (
            computation,
            old,
            first_child,
            cleanups,
            (prev_owner, prev_obs),
        )
    };
    let Some(mut computation) = computation else {
        return false;
    };

    let children_to_dispose: Vec<RawId> = state.borrow().children_of_head(first_child).collect();

    let mut result = None;
    let mut execution_started = false;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        dispose_nodes(state, children_to_dispose);
        for cleanup in cleanups {
            cleanup.call();
        }
        {
            let mut state_ref = state.borrow_mut();
            let scheduler = state_ref.scheduler.clone();
            let mut sched = scheduler.borrow_mut();
            sched.executing += 1;
            execution_started = true;
            state_ref.current_owner = Some(id);
            sched.set_observer(Some(Observer {
                scope_id: state_ref.scope_id,
                node: id,
            }));
            if let Some(node) = state_ref.nodes.get_mut(id) {
                node.state = NodeState::Clean;
            }
        }
        match &mut computation {
            Computation::Effect(callback) => callback.call(),
            Computation::Memo(callback) => result = Some(callback.compute(old.as_ref())),
        }
    }));

    let panicked = outcome.is_err();
    let equality_result = if panicked {
        Ok(None)
    } else if let Some(new_value) = result.as_ref() {
        catch_unwind(AssertUnwindSafe(|| {
            old.as_ref().is_none_or(|old| !new_value.try_eq(old))
        }))
        .map(Some)
    } else {
        Ok(None)
    };
    let failed = panicked || equality_result.is_err();
    {
        let mut state_ref = state
            .try_borrow_mut()
            .expect("ScopeState borrow failed after computation execution");
        let scheduler = state_ref.scheduler.clone();
        let mut sched = scheduler.borrow_mut();
        let now_epoch = sched.current_epoch();
        if execution_started {
            sched.executing = sched.executing.saturating_sub(1);
        }
        state_ref.set_context(previous.0);
        sched.set_observer(previous.1);
        drop(sched);

        let mut changed = false;
        if let Some(data) = state_ref.data.get_mut(id) {
            data.computation = Some(computation);
            if failed {
                if let Some(old) = old.take() {
                    data.value = Some(old);
                }
            } else if let Some(new_value) = result.take() {
                changed = match equality_result.as_ref() {
                    Ok(Some(value)) => *value,
                    _ => false,
                };
                data.value = Some(new_value);
            }
        }

        if let Some(node) = state_ref.nodes.get_mut(id) {
            node.running = false;
            node.last_computed_epoch = now_epoch;
            if failed {
                node.state = NodeState::Dirty;
            } else if changed {
                node.updated_epoch = now_epoch;
                node.version = node.version.wrapping_add(1);
            }
        }

        if changed {
            state_ref.queue_dependents(id);
        }
    }

    if let Err(panic) = outcome {
        resume_unwind(panic);
    }
    if let Err(panic) = equality_result {
        resume_unwind(panic);
    }
    drop(old);
    drop(result);
    flush_if_idle(state);
    true
}

pub(crate) fn run_initial<'scope>(state: &Rc<RefCell<ScopeState<'scope>>>, id: RawId) {
    let _ = run_node(state, id);
    flush_if_idle(state);
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
                if let Err(error) = evaluate_root(&scope_state, task.node) {
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
