//! Dependency tracking and node subscription operations on ScopeState.

use super::{
    model::{DependencyTransaction, NodeState, ReactiveEdge, ScopeState, ScopeStateInner},
    scheduler::{ExecutionContext, Observer, ScheduledTask, TargetNode, active_ctx},
};
use crate::{ReactiveError, ReactiveResult, handle::NodeKindTag, internal::NodeId};
use std::{
    collections::{HashSet, VecDeque},
    rc::Rc,
};

impl<'scope> ScopeStateInner<'scope> {
    pub(crate) fn begin_dependency_transaction(&mut self, observer: NodeId) {
        let previous = self
            .dependency_edges_of(observer)
            .map(|(_, edge)| edge.target)
            .collect();
        self.dependency_transactions.push(DependencyTransaction {
            observer,
            previous,
            current: HashSet::new(),
            removed: HashSet::new(),
        });
    }

    pub(crate) fn observe_dependency(&mut self, observer: NodeId, target: TargetNode) {
        if let Some(transaction) = self
            .dependency_transactions
            .iter_mut()
            .rev()
            .find(|transaction| transaction.observer == observer)
        {
            transaction.current.insert(target);
        }
    }

    fn ensure_dependency_scopes_available<'a>(
        &self,
        dependencies: impl IntoIterator<Item = &'a TargetNode>,
    ) -> ReactiveResult<()> {
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let mut owner_ids = HashSet::new();
        for dependency in dependencies {
            if dependency.owner_id == self.owner_id {
                continue;
            }
            if !owner_ids.insert(dependency.owner_id) {
                continue;
            }
            if let Some(dependency_scope) =
                scheduler.get_scope_for_edge_cleanup(dependency.owner_id)?
            {
                dependency_scope
                    .try_borrow_mut()
                    .map_err(|_| ReactiveError::BorrowConflict)?;
            }
        }
        Ok(())
    }

    fn remove_dependency_pair(
        &mut self,
        observer: NodeId,
        dependency: TargetNode,
    ) -> ReactiveResult<()> {
        let observer_target = TargetNode {
            owner_id: self.owner_id,
            node: observer,
        };
        if dependency.owner_id == self.owner_id {
            self.remove_dependency(observer, dependency);
            self.remove_subscriber(dependency.node, observer_target);
            return Ok(());
        }

        let dependency_scope = self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .get_scope_for_edge_cleanup(dependency.owner_id)?;
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
        observer: NodeId,
        dependency: TargetNode,
    ) -> ReactiveResult<()> {
        if !self.node_exists(observer) {
            return Ok(());
        }
        let observer_target = TargetNode {
            owner_id: self.owner_id,
            node: observer,
        };
        if dependency.owner_id == self.owner_id {
            if !self.node_exists(dependency.node) {
                return Ok(());
            }
            self.add_dependency(observer, dependency);
            self.add_subscriber(dependency.node, observer_target);
            return Ok(());
        }

        let dependency_scope = self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .get_scope_for_edge_cleanup(dependency.owner_id)?;
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

    pub(crate) fn commit_dependency_transaction(&mut self, observer: NodeId) -> ReactiveResult<()> {
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
        let removed: Vec<TargetNode> = previous.difference(&current).copied().collect();
        self.ensure_dependency_scopes_available(removed.iter())?;
        for dependency in removed {
            self.remove_dependency_pair(observer, dependency)?;
            self.dependency_transactions[index]
                .removed
                .insert(dependency);
        }
        Ok(())
    }

    pub(crate) fn finish_dependency_transaction(&mut self, observer: NodeId) {
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
        observer: NodeId,
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
            .difference(&transaction.previous)
            .copied()
            .collect();
        self.ensure_dependency_scopes_available(to_remove.iter())?;
        self.ensure_dependency_scopes_available(transaction.removed.iter())?;
        for dependency in to_remove {
            self.remove_dependency_pair(observer, dependency)?;
        }
        for dependency in transaction.removed.iter().copied() {
            self.restore_dependency_pair(observer, dependency)?;
        }
        self.dependency_transactions.remove(index);
        Ok(())
    }

    pub(crate) fn add_subscriber(&mut self, target_id: NodeId, sub_target: TargetNode) {
        let Some(adjacency) = self.adjacency.get_mut(target_id) else {
            return;
        };
        if adjacency.subscribers.contains_key(&sub_target) {
            return;
        }
        let edge_id = self.edges.insert(ReactiveEdge { target: sub_target });
        adjacency.subscribers.insert(sub_target, edge_id);
    }

    pub(crate) fn add_dependency(&mut self, observer_id: NodeId, dep_target: TargetNode) {
        let Some(adjacency) = self.adjacency.get_mut(observer_id) else {
            return;
        };
        if adjacency.dependencies.contains_key(&dep_target) {
            return;
        }
        let edge_id = self.edges.insert(ReactiveEdge { target: dep_target });
        adjacency.dependencies.insert(dep_target, edge_id);
    }

    pub(crate) fn remove_subscriber(&mut self, target_id: NodeId, sub_target: TargetNode) {
        let edge_id = self
            .adjacency
            .get_mut(target_id)
            .and_then(|adjacency| adjacency.subscribers.remove(&sub_target));
        if let Some(edge_id) = edge_id {
            self.edges.remove(edge_id);
        }
    }

    pub(crate) fn remove_dependency(&mut self, observer_id: NodeId, dep_target: TargetNode) {
        let edge_id = self
            .adjacency
            .get_mut(observer_id)
            .and_then(|adjacency| adjacency.dependencies.remove(&dep_target));
        if let Some(edge_id) = edge_id {
            self.edges.remove(edge_id);
        }
    }

    pub(crate) fn take_subscribers(&mut self, source: NodeId) -> Vec<TargetNode> {
        let entries: Vec<_> = self
            .adjacency
            .get_mut(source)
            .map(|adjacency| adjacency.subscribers.drain().collect())
            .unwrap_or_default();
        let mut targets = Vec::with_capacity(entries.len());
        for (target, edge_id) in entries {
            self.edges.remove(edge_id);
            targets.push(target);
        }
        targets
    }

    pub(crate) fn clear_dependencies(&mut self, observer_id: NodeId) -> ReactiveResult<()> {
        let dependencies: Vec<TargetNode> = self
            .dependency_edges_of(observer_id)
            .map(|(_, edge)| edge.target)
            .collect();
        self.ensure_dependency_scopes_available(dependencies.iter())?;

        let edge_ids: Vec<_> = self
            .adjacency
            .get_mut(observer_id)
            .map(|adjacency| {
                adjacency
                    .dependencies
                    .drain()
                    .map(|(_, edge_id)| edge_id)
                    .collect()
            })
            .unwrap_or_default();
        for edge_id in edge_ids {
            self.edges.remove(edge_id);
        }

        let self_sub = TargetNode {
            owner_id: self.owner_id,
            node: observer_id,
        };
        for dependency in dependencies {
            if dependency.owner_id == self.owner_id {
                self.remove_subscriber(dependency.node, self_sub);
            } else if let Some(dep_scope) = self
                .scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .get_scope_for_edge_cleanup(dependency.owner_id)?
            {
                dep_scope
                    .try_borrow_mut()
                    .map_err(|_| ReactiveError::BorrowConflict)?
                    .remove_subscriber(dependency.node, self_sub);
            }
        }
        Ok(())
    }

    fn track_pair(&mut self, observer: Observer, target: NodeId) {
        let target_sub = TargetNode {
            owner_id: observer.owner_id,
            node: observer.node,
        };
        let observer_dep = TargetNode {
            owner_id: self.owner_id,
            node: target,
        };
        self.observe_dependency(observer.node, observer_dep);
        self.add_dependency(observer.node, observer_dep);
        self.add_subscriber(target, target_sub);
    }

    fn observer_is_computation(&self, observer: Observer) -> bool {
        self.nodes
            .get(observer.node)
            .is_some_and(|node| node.is_computation())
    }

    fn observer_state(&self, context: &ExecutionContext) -> ReactiveResult<ScopeState<'scope>> {
        let observer = context.observer.ok_or(ReactiveError::NoSuchNode)?;
        context
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .get_scope_for_edge_cleanup(observer.owner_id)?
            .ok_or(ReactiveError::NoSuchNode)
    }

    /// Validate a tracked read before a dirty computation is evaluated.
    pub(crate) fn preflight_track_read(
        &self,
        target: NodeId,
    ) -> ReactiveResult<Option<ExecutionContext>> {
        let Some(ctx) = active_ctx(&self.scheduler) else {
            return Ok(None);
        };
        let Some(observer) = ctx.observer else {
            return Ok(None);
        };
        if !Rc::ptr_eq(&ctx.scheduler, &self.scheduler) {
            return Err(ReactiveError::RuntimeMismatch);
        }
        let same_scope = observer.owner_id == self.owner_id;
        let observer_scope = (!same_scope).then(|| self.observer_state(&ctx));
        if same_scope && self.observer_is_computation(observer) && observer.node == target {
            return Err(ReactiveError::Reentrant);
        }
        if !self.is_active() || !self.has_value(target) {
            return Err(ReactiveError::NoSuchNode);
        }
        if same_scope {
            if !self.observer_is_computation(observer) || observer.node == target {
                return Err(ReactiveError::Reentrant);
            }
        } else {
            let observer_scope = observer_scope.ok_or(ReactiveError::NoSuchNode)??;
            let observer_state = observer_scope
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?;
            if !observer_state.is_active()
                || !observer_state.observer_is_computation(observer)
                || observer_state.owner_id == self.owner_id && observer.node == target
            {
                return Err(ReactiveError::Reentrant);
            }
        }
        Ok(Some(ctx))
    }

    /// Record one tracked read after the source has been evaluated.
    pub(crate) fn track_read(
        &mut self,
        target: NodeId,
        ctx: &ExecutionContext,
    ) -> ReactiveResult<()> {
        let Some(observer) = ctx.observer else {
            return Ok(());
        };
        if ctx.blocked_scopes.contains(&self.owner_id) {
            return Ok(());
        }
        if !Rc::ptr_eq(&ctx.scheduler, &self.scheduler) {
            return Err(ReactiveError::RuntimeMismatch);
        }
        if !self.is_active() || !self.has_value(target) {
            return Err(ReactiveError::NoSuchNode);
        }
        let observer_target = TargetNode {
            owner_id: observer.owner_id,
            node: observer.node,
        };
        if observer.owner_id == self.owner_id {
            if observer.node == target || !self.observer_is_computation(observer) {
                return Err(ReactiveError::Reentrant);
            }
            self.track_pair(observer, target);
            return Ok(());
        }

        let observer_scope = self.observer_state(ctx)?;
        let mut observer_state = observer_scope
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !observer_state.is_active() || !observer_state.observer_is_computation(observer) {
            return Err(ReactiveError::NoSuchNode);
        }
        let observer_dep = TargetNode {
            owner_id: self.owner_id,
            node: target,
        };
        observer_state.observe_dependency(observer.node, observer_dep);
        observer_state.add_dependency(observer.node, observer_dep);
        drop(observer_state);
        self.add_subscriber(target, observer_target);
        Ok(())
    }

    pub(crate) fn queue_dependents(&mut self, source: NodeId) -> ReactiveResult<()> {
        let scheduler = self.scheduler.clone();
        let mut walk: VecDeque<TargetNode> = self
            .subscriber_edges_of(source)
            .map(|(_, edge)| edge.target)
            .collect();
        let mut visited = HashSet::new();
        let mut external_owner_ids = HashSet::new();
        let mut external_scopes = Vec::new();

        // Read the complete propagation frontier before changing any node. This
        // is the cross-scope preflight: a borrow conflict cannot leave a half-
        // marked dependency chain behind.
        while let Some(target) = walk.pop_front() {
            if !visited.insert(target) {
                continue;
            }
            if target.owner_id == self.owner_id {
                let Some(node) = self.nodes.get(target.node) else {
                    continue;
                };
                if !matches!(node.state, NodeState::Clean | NodeState::Dirty) {
                    continue;
                }
                if node.kind != NodeKindTag::Effect {
                    walk.extend(
                        self.subscriber_edges_of(target.node)
                            .map(|(_, edge)| edge.target),
                    );
                }
                continue;
            }

            let target_scope = scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .get_scope(target.owner_id)?;
            let Some(target_scope) = target_scope else {
                continue;
            };
            let state_ref = target_scope
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?;
            if !state_ref.is_active() {
                continue;
            }
            let Some(node) = state_ref.nodes.get(target.node) else {
                continue;
            };
            if !matches!(node.state, NodeState::Clean | NodeState::Dirty) {
                continue;
            }
            if node.kind != NodeKindTag::Effect {
                walk.extend(
                    state_ref
                        .subscriber_edges_of(target.node)
                        .map(|(_, edge)| edge.target),
                );
            }
            drop(state_ref);
            if external_owner_ids.insert(target.owner_id) {
                external_scopes.push(target_scope);
            }
        }

        for scope in &external_scopes {
            scope
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?;
        }

        let mut walk = self
            .subscriber_edges_of(source)
            .map(|(_, edge)| edge.target)
            .collect::<VecDeque<_>>();
        let mut visited = HashSet::new();
        while let Some(target) = walk.pop_front() {
            if !visited.insert(target) {
                continue;
            }
            if target.owner_id == self.owner_id {
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
                        scheduler
                            .try_borrow_mut()
                            .map_err(|_| ReactiveError::BorrowConflict)?
                            .enqueue_effect(ScheduledTask {
                                owner_id: target.owner_id,
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
                let target_scope = scheduler
                    .try_borrow()
                    .map_err(|_| ReactiveError::BorrowConflict)?
                    .get_scope(target.owner_id)?;
                let Some(target_scope) = target_scope else {
                    continue;
                };
                let mut state_ref = target_scope
                    .try_borrow_mut()
                    .map_err(|_| ReactiveError::BorrowConflict)?;
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
                        scheduler
                            .try_borrow_mut()
                            .map_err(|_| ReactiveError::BorrowConflict)?
                            .enqueue_effect(ScheduledTask {
                                owner_id: target.owner_id,
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
        Ok(())
    }

    pub(crate) fn is_settled(&self, id: NodeId) -> bool {
        self.nodes
            .get(id)
            .is_some_and(|node| node.state == NodeState::Clean)
            && self.scheduler.borrow().global_queue.is_empty()
            && self.scheduler.borrow().worklist.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ErrorHandlerToken, OwnerAccess, Runtime,
        owner::ScopeStorage,
        runtime::{dispose::dispose_nodes, scheduler::GlobalScheduler},
    };
    use std::marker::PhantomData;

    fn transient(runtime: &mut Runtime, f: impl for<'scope> FnOnce(OwnerAccess<'scope>)) {
        let _ = runtime.with_transient(f);
    }

    fn handler<'scope>(owner: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
        owner.error_handler(|_| {}).expect("handler registration")
    }

    #[test]
    fn transient_boundary_does_not_track_local_reads_in_an_outer_effect() {
        let mut runtime = Runtime::new();
        transient(&mut runtime, |scope| {
            let parent_scope = scope;
            let effect = scope
                .effect(
                    move || {
                        let _ = parent_scope.with_transient(|child| {
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
        transient(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let source_state = source.handle.state();
            let source_raw = source.handle.raw();

            let _ = scope.with_transient(|child| {
                let child_handler = handler(child);
                let effect = child
                    .effect(
                        move || {
                            let _ = source.get();
                            Ok(())
                        },
                        child_handler.view(),
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

                let _ = dispose_nodes(&source_state, vec![source_raw]);

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
    fn clear_dependencies_conflict_preserves_both_sides_of_the_edge() {
        let mut runtime = Runtime::new();
        transient(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let source_state = source.handle.state();
            let source_raw = source.handle.raw();

            let _ = scope.with_transient(|child| {
                let (local, _) = child.signal(0i32).expect("fallible reactive creation");
                let child_handler = handler(child);
                let effect = child
                    .effect(
                        move || {
                            let _ = source.get();
                            Ok(())
                        },
                        child_handler.view(),
                    )
                    .expect("effect should initialize");
                let child_state = local.handle.state();
                let effect_raw = effect.handle.raw();
                let (child_owner_id, source_owner_id) = {
                    let child_state_ref = child_state.borrow();
                    let source_state_ref = source_state.borrow();
                    (child_state_ref.owner_id, source_state_ref.owner_id)
                };
                let source_borrow = source_state.borrow_mut();
                let mut child_borrow = child_state.borrow_mut();
                let result = child_borrow.clear_dependencies(effect_raw);

                assert_eq!(result, Err(ReactiveError::BorrowConflict));
                assert_eq!(
                    source_borrow
                        .subscriber_edges_of(source_raw)
                        .filter(|(_, edge)| {
                            edge.target
                                == TargetNode {
                                    owner_id: child_owner_id,
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
                                    owner_id: source_owner_id,
                                    node: source_raw,
                                }
                        })
                        .count(),
                    1
                );
                drop(child_borrow);
                drop(source_borrow);

                let _ = child_state.borrow_mut().clear_dependencies(effect_raw);
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
        let source_scope = OwnerAccess {
            storage: &source_storage,
            marker: PhantomData,
        };
        let observer_scope = OwnerAccess {
            storage: &observer_storage,
            marker: PhantomData,
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
        let source_state = source_storage.owner_token().state();
        let observer_state = observer_storage.owner_token().state();
        let source_raw = source.handle.raw();
        let effect_raw = effect.handle.raw();
        let observer_target = TargetNode {
            owner_id: observer_state.borrow().owner_id,
            node: effect_raw,
        };
        let source_target = TargetNode {
            owner_id: source_state.borrow().owner_id,
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
        let observer_outcome = observer_storage.dispose_untracked();
        let source_outcome = source_storage.dispose_untracked();
        assert!(observer_outcome.released);
        assert!(observer_outcome.error.is_none());
        assert!(source_outcome.released);
        assert!(source_outcome.error.is_none());
    }
}
