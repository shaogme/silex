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
        let first_dep = if let Some(node) = self.nodes.get_mut(observer_id) {
            let first = node.first_dependency;
            node.first_dependency = EdgeId::DANGLING;
            first
        } else {
            EdgeId::DANGLING
        };

        let self_sub = TargetNode {
            scope_id: self.scope_id,
            node: observer_id,
        };

        let mut curr_edge = first_dep;
        while curr_edge.is_valid() {
            let Some(edge) = self.edges.remove(curr_edge) else {
                break;
            };
            let dep = edge.target;
            if dep.scope_id == self.scope_id {
                self.remove_subscriber(dep.node, self_sub);
            } else {
                let dep_scope = self.scheduler.borrow().get_scope(dep.scope_id);
                if let Some(dep_scope) = dep_scope {
                    if let Ok(mut dep_state) = dep_scope.try_borrow_mut() {
                        dep_state.remove_subscriber(dep.node, self_sub);
                    }
                }
            }
            curr_edge = edge.next;
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
            if let Some(obs_node) = self.nodes.get(observer_node) {
                if !obs_node.is_computation() {
                    return;
                }
            }
            self.add_subscriber(target, target_sub);
            self.add_dependency(observer_node, observer_dep);
            return;
        }

        // Cross-scope dependency
        self.add_subscriber(target, target_sub);

        let obs_scope = self.scheduler.borrow().get_scope(observer.scope_id);
        if let Some(obs_scope) = obs_scope {
            if let Ok(mut obs_state) = obs_scope.try_borrow_mut() {
                if let Some(obs_node) = obs_state.nodes.get(observer.node) {
                    if obs_node.is_computation() {
                        obs_state.add_dependency(observer.node, observer_dep);
                    }
                }
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
                if node.state != NodeState::Clean {
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
                let Ok(mut state_ref) = target_scope.try_borrow_mut() else {
                    continue;
                };
                let Some(node) = state_ref.nodes.get_mut(target.node) else {
                    continue;
                };
                if node.state != NodeState::Clean {
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
