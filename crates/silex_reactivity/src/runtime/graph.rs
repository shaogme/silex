//! Dependency tracking and node subscription operations on ScopeState.

use super::{
    model::{EdgeId, NodeState, ReactiveEdge, ScopeState},
    scheduler::{ScheduledTask, TargetNode},
};
use crate::{handle::NodeKindTag, internal::RawId};
use std::collections::VecDeque;

impl<'scope> ScopeState<'scope> {
    pub(crate) fn add_subscriber(&mut self, target_id: RawId, sub_target: TargetNode) {
        if self
            .subscriber_edges_of(target_id)
            .any(|(_, edge)| edge.target == sub_target)
        {
            return;
        }
        let old_first = self
            .nodes
            .get(target_id)
            .map(|n| n.first_subscriber)
            .unwrap_or(EdgeId::DANGLING);
        let edge_id = self.edges.insert(ReactiveEdge {
            target: sub_target,
            next: old_first,
        });
        if let Some(node) = self.nodes.get_mut(target_id) {
            node.first_subscriber = edge_id;
        }
    }

    pub(crate) fn add_dependency(&mut self, observer_id: RawId, dep_target: TargetNode) {
        if self
            .dependency_edges_of(observer_id)
            .any(|(_, edge)| edge.target == dep_target)
        {
            return;
        }
        let old_first = self
            .nodes
            .get(observer_id)
            .map(|n| n.first_dependency)
            .unwrap_or(EdgeId::DANGLING);
        let edge_id = self.edges.insert(ReactiveEdge {
            target: dep_target,
            next: old_first,
        });
        if let Some(node) = self.nodes.get_mut(observer_id) {
            node.first_dependency = edge_id;
        }
    }

    pub(crate) fn remove_subscriber(&mut self, target_id: RawId, sub_target: TargetNode) {
        let Some(node) = self.nodes.get(target_id) else {
            return;
        };
        let mut prev_edge = EdgeId::DANGLING;
        let mut curr_edge = node.first_subscriber;
        while curr_edge.is_valid() {
            let Some(edge) = self.edges.get(curr_edge).copied() else {
                break;
            };
            if edge.target == sub_target {
                if prev_edge.is_dangling() {
                    if let Some(node) = self.nodes.get_mut(target_id) {
                        node.first_subscriber = edge.next;
                    }
                } else if let Some(prev) = self.edges.get_mut(prev_edge) {
                    prev.next = edge.next;
                }
                self.edges.remove(curr_edge);
                break;
            }
            prev_edge = curr_edge;
            curr_edge = edge.next;
        }
    }

    pub(crate) fn clear_dependencies(&mut self, observer_id: RawId) {
        let self_sub = TargetNode {
            scope_id: self.scope_id,
            node: observer_id,
        };

        let dependencies: Vec<(EdgeId, ReactiveEdge)> =
            self.dependency_edges_of(observer_id).collect();
        for (_, edge) in &dependencies {
            if edge.target.scope_id != self.scope_id
                && let Some(dep_scope) = self.scheduler.borrow().get_scope(edge.target.scope_id)
            {
                dep_scope
                    .try_borrow_mut()
                    .expect("dep_scope borrow failed during clear_dependencies");
            }
        }

        if let Some(node) = self.nodes.get_mut(observer_id) {
            node.first_dependency = EdgeId::DANGLING;
        }

        for (edge_id, _) in dependencies {
            let Some(edge) = self.edges.remove(edge_id) else {
                break;
            };
            let dep = edge.target;
            if dep.scope_id == self.scope_id {
                self.remove_subscriber(dep.node, self_sub);
            } else {
                let dep_scope = self.scheduler.borrow().get_scope(dep.scope_id);
                if let Some(dep_scope) = dep_scope {
                    let mut dep_state = dep_scope
                        .try_borrow_mut()
                        .expect("dep_scope borrow failed during clear_dependencies");
                    dep_state.remove_subscriber(dep.node, self_sub);
                }
            }
        }
    }

    pub(crate) fn track(&mut self, target: RawId) {
        let Some(observer) = self.scheduler.borrow().observer() else {
            return;
        };
        if !self.has_value(target) {
            return;
        }

        let target_sub = TargetNode {
            scope_id: observer.scope_id,
            node: observer.node,
        };
        let observer_dep = TargetNode {
            scope_id: self.scope_id,
            node: target,
        };

        if observer.scope_id == self.scope_id {
            let observer_node = observer.node;
            if observer_node == target || !self.node_exists(observer_node) {
                return;
            }
            if let Some(obs_node) = self.nodes.get(observer_node)
                && !obs_node.is_computation()
            {
                return;
            }
            self.add_subscriber(target, target_sub);
            self.add_dependency(observer_node, observer_dep);
            return;
        }

        // Cross-scope dependency
        let obs_scope = self.scheduler.borrow().get_scope(observer.scope_id);
        if let Some(obs_scope) = obs_scope {
            let mut obs_state = obs_scope
                .try_borrow_mut()
                .expect("obs_scope borrow failed during track");
            if let Some(obs_node) = obs_state.nodes.get(observer.node)
                && obs_node.is_computation()
            {
                obs_state.add_dependency(observer.node, observer_dep);
                drop(obs_state);
                self.add_subscriber(target, target_sub);
            }
        }
    }

    pub(crate) fn track_many(&mut self, targets: &[RawId]) {
        for &target in targets {
            self.track(target);
        }
    }

    pub(crate) fn queue_dependents(&mut self, source: RawId) {
        let mut walk: VecDeque<TargetNode> = self
            .subscriber_edges_of(source)
            .map(|(_, edge)| edge.target)
            .collect();

        let scheduler = self.scheduler.clone();

        while let Some(target) = walk.pop_front() {
            if target.scope_id == self.scope_id {
                let Some(node) = self.nodes.get_mut(target.node) else {
                    continue;
                };
                if !matches!(node.state, NodeState::Clean | NodeState::Dirty) {
                    continue;
                }
                node.state = NodeState::Check;
                if node.kind == NodeKindTag::Effect {
                    if !node.queued {
                        node.queued = true;
                        scheduler.borrow_mut().enqueue_effect(ScheduledTask {
                            scope_id: target.scope_id,
                            node: target.node,
                        });
                    }
                } else {
                    walk.extend(
                        self.subscriber_edges_of(target.node)
                            .map(|(_, edge)| edge.target),
                    );
                }
            } else {
                let target_scope = scheduler.borrow().get_scope(target.scope_id);
                let Some(target_scope) = target_scope else {
                    continue;
                };
                let mut state_ref = target_scope
                    .try_borrow_mut()
                    .expect("target_scope borrow failed during queue_dependents");
                let Some(node) = state_ref.nodes.get_mut(target.node) else {
                    continue;
                };
                if !matches!(node.state, NodeState::Clean | NodeState::Dirty) {
                    continue;
                }
                node.state = NodeState::Check;
                if node.kind == NodeKindTag::Effect {
                    if !node.queued {
                        node.queued = true;
                        drop(state_ref);
                        scheduler.borrow_mut().enqueue_effect(ScheduledTask {
                            scope_id: target.scope_id,
                            node: target.node,
                        });
                    }
                } else {
                    walk.extend(
                        state_ref
                            .subscriber_edges_of(target.node)
                            .map(|(_, edge)| edge.target),
                    );
                }
            }
        }
    }

    pub(crate) fn is_settled(&self, id: RawId) -> bool {
        self.nodes
            .get(id)
            .is_some_and(|node| node.state == NodeState::Clean)
            && self.scheduler.borrow().global_queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Runtime, Scope, runtime::scheduler::Observer};
    use std::{panic::AssertUnwindSafe, panic::catch_unwind};

    fn child(runtime: &mut Runtime, f: impl for<'scope> FnOnce(&'scope Scope<'scope>)) {
        let root = runtime.run(|root| root.child(f));
        drop(root);
    }

    #[test]
    fn track_conflict_does_not_leave_a_subscriber_half_edge() {
        let mut runtime = Runtime::new();
        child(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32);
            let (target, _) = scope.signal(1i32);
            let source_state = source.handle.state();

            scope.child(|child| {
                let (local, _) = child.signal(0i32);
                let effect = child.effect(move || {
                    let _ = source.get();
                });
                let child_state = local.handle.state();
                let (scheduler, child_scope_id) = {
                    let state = child_state.borrow();
                    (state.scheduler.clone(), state.scope_id)
                };
                scheduler.borrow_mut().set_observer(Some(Observer {
                    scope_id: child_scope_id,
                    node: effect.handle.raw(),
                }));

                let target_raw = target.handle.raw();
                let effect_raw = effect.handle.raw();
                let child_borrow = child_state.borrow_mut();
                let mut source_borrow = source_state.borrow_mut();
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    source_borrow.track(target_raw);
                }));

                assert!(panic.is_err());
                assert_eq!(
                    source_borrow
                        .subscriber_edges_of(target_raw)
                        .filter(|(_, edge)| {
                            edge.target
                                == TargetNode {
                                    scope_id: child_scope_id,
                                    node: effect_raw,
                                }
                        })
                        .count(),
                    0
                );
                assert_eq!(
                    child_borrow
                        .dependency_edges_of(effect_raw)
                        .filter(|(_, edge)| edge.target.node == target_raw)
                        .count(),
                    0
                );
                drop(source_borrow);
                drop(child_borrow);

                source_state.borrow_mut().track(target_raw);
                assert_eq!(
                    source_state
                        .borrow()
                        .subscriber_edges_of(target_raw)
                        .filter(|(_, edge)| {
                            edge.target
                                == TargetNode {
                                    scope_id: child_scope_id,
                                    node: effect_raw,
                                }
                        })
                        .count(),
                    1
                );
                scheduler.borrow_mut().set_observer(None);
            });
        });
    }

    #[test]
    fn clear_dependencies_conflict_preserves_both_sides_of_the_edge() {
        let mut runtime = Runtime::new();
        child(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32);
            let source_state = source.handle.state();
            let source_raw = source.handle.raw();

            scope.child(|child| {
                let (local, _) = child.signal(0i32);
                let effect = child.effect(move || {
                    let _ = source.get();
                });
                let child_state = local.handle.state();
                let effect_raw = effect.handle.raw();
                let (child_scope_id, source_scope_id) = {
                    let child_state_ref = child_state.borrow();
                    let source_state_ref = source_state.borrow();
                    (child_state_ref.scope_id, source_state_ref.scope_id)
                };
                let source_borrow = source_state.borrow_mut();
                let mut child_borrow = child_state.borrow_mut();
                let panic = catch_unwind(AssertUnwindSafe(|| {
                    child_borrow.clear_dependencies(effect_raw);
                }));

                assert!(panic.is_err());
                assert_eq!(
                    source_borrow
                        .subscriber_edges_of(source_raw)
                        .filter(|(_, edge)| {
                            edge.target
                                == TargetNode {
                                    scope_id: child_scope_id,
                                    node: effect_raw,
                                }
                        })
                        .count(),
                    1
                );
                assert_eq!(
                    child_borrow
                        .dependency_edges_of(effect_raw)
                        .filter(|(_, edge)| {
                            edge.target
                                == TargetNode {
                                    scope_id: source_scope_id,
                                    node: source_raw,
                                }
                        })
                        .count(),
                    1
                );
                drop(child_borrow);
                drop(source_borrow);

                child_state.borrow_mut().clear_dependencies(effect_raw);
                assert_eq!(
                    source_state
                        .borrow()
                        .subscriber_edges_of(source_raw)
                        .count(),
                    0
                );
                assert_eq!(
                    child_state.borrow().dependency_edges_of(effect_raw).count(),
                    0
                );
            });
        });
    }
}
