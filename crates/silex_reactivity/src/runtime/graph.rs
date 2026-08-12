//! Dependency tracking and node subscription operations on ScopeState.

use super::{
    model::{DependencyTransaction, EdgeId, NodeState, ReactiveEdge, ScopeState},
    scheduler::{ScheduledTask, TargetNode},
};
use crate::{ReactiveError, ReactiveResult, handle::NodeKindTag, internal::RawId};
use std::collections::VecDeque;

impl<'scope> ScopeState<'scope> {
    pub(crate) fn begin_dependency_transaction(&mut self, observer: RawId) {
        let previous = self
            .dependency_edges_of(observer)
            .map(|(_, edge)| edge.target)
            .collect();
        self.dependency_transactions.push(DependencyTransaction {
            observer,
            previous,
            current: Vec::new(),
            removed: Vec::new(),
        });
    }

    pub(crate) fn observe_dependency(&mut self, observer: RawId, target: TargetNode) {
        if let Some(transaction) = self
            .dependency_transactions
            .iter_mut()
            .rev()
            .find(|transaction| transaction.observer == observer)
            && !transaction.current.contains(&target)
        {
            transaction.current.push(target);
        }
    }

    fn ensure_dependency_scopes_available(
        &self,
        dependencies: &[TargetNode],
    ) -> ReactiveResult<()> {
        let scheduler = self.scheduler.borrow();
        for dependency in dependencies {
            if dependency.scope_id == self.scope_id {
                continue;
            }
            if let Some(dependency_scope) = scheduler.get_scope(dependency.scope_id) {
                dependency_scope
                    .try_borrow_mut()
                    .map_err(|_| ReactiveError::BorrowConflict)?;
            }
        }
        Ok(())
    }

    fn remove_dependency_pair(
        &mut self,
        observer: RawId,
        dependency: TargetNode,
    ) -> ReactiveResult<()> {
        let observer_target = TargetNode {
            scope_id: self.scope_id,
            node: observer,
        };
        if dependency.scope_id == self.scope_id {
            self.remove_dependency(observer, dependency);
            self.remove_subscriber(dependency.node, observer_target);
            return Ok(());
        }

        let dependency_scope = self.scheduler.borrow().get_scope(dependency.scope_id);
        let Some(dependency_scope) = dependency_scope else {
            self.remove_dependency(observer, dependency);
            return Ok(());
        };
        let mut dependency_state = dependency_scope
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        self.remove_dependency(observer, dependency);
        dependency_state.remove_subscriber(dependency.node, observer_target);
        Ok(())
    }

    fn restore_dependency_pair(
        &mut self,
        observer: RawId,
        dependency: TargetNode,
    ) -> ReactiveResult<()> {
        if !self.node_exists(observer) {
            return Ok(());
        }
        let observer_target = TargetNode {
            scope_id: self.scope_id,
            node: observer,
        };
        if dependency.scope_id == self.scope_id {
            if !self.node_exists(dependency.node) {
                return Ok(());
            }
            self.add_dependency(observer, dependency);
            self.add_subscriber(dependency.node, observer_target);
            return Ok(());
        }

        let dependency_scope = self.scheduler.borrow().get_scope(dependency.scope_id);
        let Some(dependency_scope) = dependency_scope else {
            return Ok(());
        };
        let mut dependency_state = dependency_scope
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !dependency_state.node_exists(dependency.node) {
            return Ok(());
        }
        self.add_dependency(observer, dependency);
        dependency_state.add_subscriber(dependency.node, observer_target);
        Ok(())
    }

    pub(crate) fn commit_dependency_transaction(&mut self, observer: RawId) -> ReactiveResult<()> {
        let Some(index) = self
            .dependency_transactions
            .iter()
            .rposition(|transaction| transaction.observer == observer)
        else {
            return Ok(());
        };

        let (previous, current) = {
            let transaction = &self.dependency_transactions[index];
            (transaction.previous.clone(), transaction.current.clone())
        };
        let removed: Vec<TargetNode> = previous
            .into_iter()
            .filter(|dependency| !current.contains(dependency))
            .collect();
        self.ensure_dependency_scopes_available(&removed)?;
        for dependency in removed {
            self.remove_dependency_pair(observer, dependency)?;
            self.dependency_transactions[index].removed.push(dependency);
        }
        Ok(())
    }

    pub(crate) fn finish_dependency_transaction(&mut self, observer: RawId) {
        if let Some(index) = self
            .dependency_transactions
            .iter()
            .rposition(|transaction| transaction.observer == observer)
        {
            self.dependency_transactions.remove(index);
        }
    }

    pub(crate) fn rollback_dependency_transaction(
        &mut self,
        observer: RawId,
    ) -> ReactiveResult<()> {
        let Some(index) = self
            .dependency_transactions
            .iter()
            .rposition(|transaction| transaction.observer == observer)
        else {
            return Ok(());
        };
        let transaction = self.dependency_transactions[index].clone();
        let to_remove: Vec<TargetNode> = transaction
            .current
            .iter()
            .copied()
            .filter(|dependency| !transaction.previous.contains(dependency))
            .collect();
        self.ensure_dependency_scopes_available(&to_remove)?;
        self.ensure_dependency_scopes_available(&transaction.removed)?;
        for dependency in to_remove {
            self.remove_dependency_pair(observer, dependency)?;
        }
        for dependency in transaction.removed {
            self.restore_dependency_pair(observer, dependency)?;
        }
        self.dependency_transactions.remove(index);
        Ok(())
    }

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

    pub(crate) fn remove_dependency(&mut self, observer_id: RawId, dep_target: TargetNode) {
        let Some(node) = self.nodes.get(observer_id) else {
            return;
        };
        let mut prev_edge = EdgeId::DANGLING;
        let mut curr_edge = node.first_dependency;
        while curr_edge.is_valid() {
            let Some(edge) = self.edges.get(curr_edge).copied() else {
                break;
            };
            if edge.target == dep_target {
                if prev_edge.is_dangling() {
                    if let Some(node) = self.nodes.get_mut(observer_id) {
                        node.first_dependency = edge.next;
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
        let observer = {
            let scheduler = self.scheduler.borrow();
            let Some(observer) = scheduler.observer() else {
                return;
            };
            if !scheduler.allows_tracking(observer, self.scope_id) {
                return;
            }
            observer
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
            self.observe_dependency(observer_node, observer_dep);
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
                obs_state.observe_dependency(observer.node, observer_dep);
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
    use crate::{
        ErrorHandler, Runtime, Scope,
        runtime::{
            dispose::dispose_nodes,
            scheduler::{GlobalScheduler, Observer, ObserverFrame},
        },
        scope::ScopeStorage,
    };
    use std::{panic::AssertUnwindSafe, panic::catch_unwind};

    fn child(runtime: &mut Runtime, f: impl for<'scope> FnOnce(Scope<'scope>)) {
        let _ = runtime.child(f);
    }

    fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
        scope.error_handler(|_| {}).expect("handler registration")
    }

    #[test]
    fn child_boundary_does_not_track_local_reads_in_an_outer_effect() {
        let mut runtime = Runtime::new();
        child(&mut runtime, |scope| {
            let parent_scope = scope;
            let effect = scope
                .effect(
                    move || {
                        let _ = parent_scope.child(|child| {
                            let (local, _) =
                                child.signal(0i32).expect("fallible reactive creation");
                            let local_state = local.handle.state();
                            let local_raw = local.handle.raw();

                            assert_eq!(local.get(), Ok(0));
                            assert_eq!(
                                local_state.borrow().subscriber_edges_of(local_raw).count(),
                                0
                            );
                        });
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(
                effect
                    .handle
                    .state()
                    .borrow()
                    .dependency_edges_of(effect.handle.raw())
                    .count(),
                0
            );
        });
    }

    #[test]
    fn disposing_source_removes_cross_scope_observer_dependency() {
        let mut runtime = Runtime::new();
        child(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let source_state = source.handle.state();
            let source_raw = source.handle.raw();

            let _ = scope.child(|child| {
                let effect = child
                    .effect(
                        move || {
                            let _ = source.get();
                            Ok(())
                        },
                        handler(child),
                    )
                    .expect("effect should initialize");
                let effect_state = effect.handle.state();
                let effect_raw = effect.handle.raw();

                assert_eq!(
                    source_state
                        .borrow()
                        .subscriber_edges_of(source_raw)
                        .count(),
                    1
                );
                assert_eq!(
                    effect_state
                        .borrow()
                        .dependency_edges_of(effect_raw)
                        .count(),
                    1
                );

                dispose_nodes(&source_state, vec![source_raw]);

                assert_eq!(
                    effect_state
                        .borrow()
                        .dependency_edges_of(effect_raw)
                        .count(),
                    0
                );
            });
        });
    }

    #[test]
    fn track_conflict_does_not_leave_a_subscriber_half_edge() {
        let mut runtime = Runtime::new();
        child(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let (target, _) = scope.signal(1i32).expect("fallible reactive creation");
            let source_state = source.handle.state();

            let _ = scope.child(|child| {
                let (local, _) = child.signal(0i32).expect("fallible reactive creation");
                let effect = child
                    .effect(
                        move || {
                            let _ = source.get();
                            Ok(())
                        },
                        handler(child),
                    )
                    .expect("effect should initialize");
                let child_state = local.handle.state();
                let (scheduler, child_scope_id) = {
                    let state = child_state.borrow();
                    (state.scheduler.clone(), state.scope_id)
                };
                let observer_frame = ObserverFrame::push(
                    scheduler.clone(),
                    Some(Observer {
                        scope_id: child_scope_id,
                        node: effect.handle.raw(),
                    }),
                );

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
                drop(observer_frame);
            });
        });
    }

    #[test]
    fn clear_dependencies_conflict_preserves_both_sides_of_the_edge() {
        let mut runtime = Runtime::new();
        child(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let source_state = source.handle.state();
            let source_raw = source.handle.raw();

            let _ = scope.child(|child| {
                let (local, _) = child.signal(0i32).expect("fallible reactive creation");
                let effect = child
                    .effect(
                        move || {
                            let _ = source.get();
                            Ok(())
                        },
                        handler(child),
                    )
                    .expect("effect should initialize");
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

    #[test]
    fn transaction_commit_conflict_preserves_both_sides_of_the_edge() {
        let scheduler = GlobalScheduler::new();
        let source_storage = ScopeStorage::new(scheduler.clone());
        let observer_storage = ScopeStorage::new(scheduler);
        let source_scope = Scope {
            storage: &source_storage,
            _marker: std::marker::PhantomData,
        };
        let observer_scope = Scope {
            storage: &observer_storage,
            _marker: std::marker::PhantomData,
        };
        let (source, _) = source_scope
            .signal(0_i32)
            .expect("fallible reactive creation");
        let effect = observer_scope
            .effect(
                move || {
                    let _ = source.get();
                    Ok(())
                },
                handler(observer_scope),
            )
            .expect("effect should initialize");
        let source_state = unsafe { source_storage.typed_state() };
        let observer_state = unsafe { observer_storage.typed_state() };
        let source_raw = source.handle.raw();
        let effect_raw = effect.handle.raw();
        let observer_target = TargetNode {
            scope_id: observer_state.borrow().scope_id,
            node: effect_raw,
        };
        let source_target = TargetNode {
            scope_id: source_state.borrow().scope_id,
            node: source_raw,
        };

        observer_state
            .borrow_mut()
            .begin_dependency_transaction(effect_raw);
        let source_borrow = source_state.borrow_mut();
        let mut observer_borrow = observer_state.borrow_mut();
        assert_eq!(
            observer_borrow.commit_dependency_transaction(effect_raw),
            Err(ReactiveError::BorrowConflict)
        );
        assert_eq!(
            observer_borrow
                .dependency_edges_of(effect_raw)
                .filter(|(_, edge)| edge.target == source_target)
                .count(),
            1
        );
        assert_eq!(
            source_borrow
                .subscriber_edges_of(source_raw)
                .filter(|(_, edge)| edge.target == observer_target)
                .count(),
            1
        );
        drop(observer_borrow);
        drop(source_borrow);

        observer_state
            .borrow_mut()
            .rollback_dependency_transaction(effect_raw)
            .expect("rollback should discard the pending transaction");
        observer_storage.dispose_untracked();
        source_storage.dispose_untracked();
    }
}
