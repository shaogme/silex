//! Dependency tracking and node subscription operations on ScopeState.

use super::{
    model::{
        DependencyOrigin, DependencyTransaction, NodeState, PropagationScratch, ScopeState,
        ScopeStateInner,
    },
    scheduler::{
        ExecutionContext, Observer, ScheduledTask, TargetNode, active_ctx, active_observer_contexts,
    },
};
use crate::{ReactiveError, ReactiveResult, handle::NodeKindTag, internal::NodeId};
use smallvec::SmallVec;
use std::{
    collections::HashSet,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

impl<'scope> ScopeStateInner<'scope> {
    pub(crate) fn begin_dependency_transaction(&mut self, observer: NodeId) {
        let current = self.dependency_buffer_pool.pop().unwrap_or_default();
        let pending_sources = self.target_buffer_pool.pop().unwrap_or_default();
        self.dependency_transactions.push(DependencyTransaction {
            observer,
            current,
            pending_sources,
        });
    }

    pub(crate) fn observe_dependency(&mut self, observer: NodeId, target: TargetNode) {
        let origin = if self
            .adjacency
            .get(observer)
            .is_some_and(|adjacency| adjacency.dependencies.contains(&target))
        {
            DependencyOrigin::Existing
        } else {
            DependencyOrigin::New
        };
        if let Some(transaction) = self
            .dependency_transactions
            .iter_mut()
            .rev()
            .find(|transaction| transaction.observer == observer)
        {
            transaction.current.insert(target, origin);
        }
    }

    fn ensure_dependency_scopes_available(
        &self,
        dependencies: impl IntoIterator<Item = TargetNode>,
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

    fn add_dependency_pair(
        &mut self,
        observer: NodeId,
        dependency: TargetNode,
    ) -> ReactiveResult<()> {
        if !self.node_exists(observer) {
            return Err(ReactiveError::NoSuchNode);
        }
        let observer_target = TargetNode {
            owner_id: self.owner_id,
            node: observer,
        };
        if dependency.owner_id == self.owner_id {
            if !self.node_exists(dependency.node) {
                return Err(ReactiveError::NoSuchNode);
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
            return Err(ReactiveError::NoSuchNode);
        };
        let mut dependency_state = dependency_scope
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if !dependency_state.node_exists(dependency.node) {
            return Err(ReactiveError::NoSuchNode);
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

        let (removed, additions) = {
            let Some(transaction) = self.dependency_transactions.get(index) else {
                return Err(ReactiveError::InvariantViolation);
            };
            let removed: SmallVec<[_; 8]> = self
                .dependency_edges_of(observer)
                .filter(|target| !transaction.current.contains(*target))
                .collect();
            let additions: SmallVec<[_; 8]> = transaction
                .current
                .iter()
                .filter(|target| !self.has_dependency(observer, *target))
                .collect();
            (removed, additions)
        };
        self.ensure_dependency_scopes_available(
            removed.iter().copied().chain(additions.iter().copied()),
        )?;
        for dependency in removed {
            self.remove_dependency_pair(observer, dependency)?;
        }
        for dependency in additions {
            self.add_dependency_pair(observer, dependency)?;
        }
        Ok(())
    }

    pub(crate) fn finish_dependency_transaction(
        &mut self,
        observer: NodeId,
    ) -> ReactiveResult<SmallVec<[TargetNode; 8]>> {
        if let Some(index) = self
            .dependency_transactions
            .iter()
            .rposition(|transaction| transaction.observer == observer)
        {
            let transaction = self.dependency_transactions.remove(index);
            return Ok(self.recycle_dependency_transaction(transaction));
        }
        Ok(SmallVec::new())
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
        let transaction = self.dependency_transactions.remove(index);
        self.recycle_dependency_transaction(transaction);
        Ok(())
    }

    fn recycle_dependency_transaction(
        &mut self,
        mut transaction: DependencyTransaction,
    ) -> SmallVec<[TargetNode; 8]> {
        let pending_sources = transaction.pending_sources.take_targets();
        transaction.current.reset();
        self.dependency_buffer_pool.push(transaction.current);
        transaction.pending_sources.reset();
        self.target_buffer_pool.push(transaction.pending_sources);
        pending_sources
    }

    pub(crate) fn add_subscriber(&mut self, target_id: NodeId, sub_target: TargetNode) {
        let Some(adjacency) = self.adjacency.get_mut(target_id) else {
            return;
        };
        adjacency.subscribers.insert(sub_target);
    }

    fn has_dependency(&self, observer_id: NodeId, target: TargetNode) -> bool {
        self.adjacency
            .get(observer_id)
            .is_some_and(|adjacency| adjacency.dependencies.contains(&target))
    }

    pub(crate) fn add_dependency(&mut self, observer_id: NodeId, dep_target: TargetNode) {
        let Some(adjacency) = self.adjacency.get_mut(observer_id) else {
            return;
        };
        adjacency.dependencies.insert(dep_target);
    }

    pub(crate) fn remove_subscriber(&mut self, target_id: NodeId, sub_target: TargetNode) {
        if let Some(adjacency) = self.adjacency.get_mut(target_id) {
            adjacency.subscribers.remove(&sub_target);
        }
    }

    pub(crate) fn remove_dependency(&mut self, observer_id: NodeId, dep_target: TargetNode) {
        if let Some(adjacency) = self.adjacency.get_mut(observer_id) {
            adjacency.dependencies.remove(&dep_target);
        }
    }

    pub(crate) fn take_subscribers_into(&mut self, source: NodeId, targets: &mut Vec<TargetNode>) {
        targets.clear();
        if let Some(adjacency) = self.adjacency.get_mut(source) {
            targets.extend(adjacency.subscribers.drain());
        }
    }

    pub(crate) fn clear_dependencies(&mut self, observer_id: NodeId) -> ReactiveResult<()> {
        let dependencies: Vec<TargetNode> = self.dependency_edges_of(observer_id).collect();
        self.ensure_dependency_scopes_available(dependencies.iter().copied())?;

        if let Some(adjacency) = self.adjacency.get_mut(observer_id) {
            adjacency.dependencies.clear();
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
        let observer_dep = TargetNode {
            owner_id: self.owner_id,
            node: target,
        };
        self.observe_dependency(observer.node, observer_dep);
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
        let Some(ctx) = active_ctx(&self.scheduler)? else {
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
        if !self.is_active()? || !self.has_value(target)? {
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
            if !observer_state.is_active()?
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
        if !self.is_active()? || !self.has_value(target)? {
            return Err(ReactiveError::NoSuchNode);
        }
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
        if !observer_state.is_active()? || !observer_state.observer_is_computation(observer) {
            return Err(ReactiveError::NoSuchNode);
        }
        let observer_dep = TargetNode {
            owner_id: self.owner_id,
            node: target,
        };
        observer_state.observe_dependency(observer.node, observer_dep);
        Ok(())
    }

    fn stage_pending_source(&mut self, source: NodeId) -> ReactiveResult<()> {
        let source_target = TargetNode {
            owner_id: self.owner_id,
            node: source,
        };
        let scheduler = self.scheduler.clone();
        let contexts = active_observer_contexts(&scheduler)?;
        for context in contexts {
            if context.blocked_scopes.contains(&self.owner_id) {
                continue;
            }
            let Some(observer) = context.observer else {
                continue;
            };
            if observer.owner_id == self.owner_id {
                self.stage_pending_source_for_observer(observer, source_target);
                continue;
            }
            let observer_scope = scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .get_scope_for_edge_cleanup(observer.owner_id)?;
            if let Some(observer_scope) = observer_scope {
                observer_scope
                    .try_borrow_mut()
                    .map_err(|_| ReactiveError::BorrowConflict)?
                    .stage_pending_source_for_observer(observer, source_target);
            }
        }
        Ok(())
    }

    fn stage_pending_source_for_observer(&mut self, observer: Observer, source: TargetNode) {
        if let Some(transaction) = self
            .dependency_transactions
            .iter_mut()
            .rev()
            .find(|transaction| transaction.observer == observer.node)
            && transaction.current.is_new(source)
        {
            transaction.pending_sources.insert(source);
        }
    }

    pub(crate) fn queue_dependents(&mut self, source: NodeId) -> ReactiveResult<()> {
        let mut scratch = if let Some(scratch) = self.propagation_scratch_pool.pop() {
            #[cfg(feature = "test-support")]
            {
                self.scratch_stats.propagation_pool_hits =
                    self.scratch_stats.propagation_pool_hits.saturating_add(1);
            }
            scratch
        } else {
            #[cfg(feature = "test-support")]
            {
                self.scratch_stats.propagation_pool_misses =
                    self.scratch_stats.propagation_pool_misses.saturating_add(1);
            }
            PropagationScratch::default()
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.queue_dependents_with_scratch(source, &mut scratch)
        }));
        #[cfg(feature = "test-support")]
        self.record_propagation_scratch(&scratch);
        scratch.reset();
        self.propagation_scratch_pool.push(scratch);
        match result {
            Ok(result) => result,
            Err(panic) => resume_unwind(panic),
        }
    }

    fn queue_dependents_with_scratch(
        &mut self,
        source: NodeId,
        scratch: &mut PropagationScratch<'scope>,
    ) -> ReactiveResult<()> {
        self.stage_pending_source(source)?;
        let scheduler = self.scheduler.clone();
        scratch.frontier.extend(self.subscriber_edges_of(source));

        // Read the complete propagation frontier before changing any node. This
        // is the cross-scope preflight: a borrow conflict cannot leave a half-
        // marked dependency chain behind.
        let mut cursor = 0;
        while cursor < scratch.frontier.len() {
            let Some(target) = scratch.frontier.get(cursor).copied() else {
                break;
            };
            cursor = cursor.saturating_add(1);
            if !scratch.visited.insert(target) {
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
                    scratch
                        .frontier
                        .extend(self.subscriber_edges_of(target.node));
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
            if !state_ref.is_active()? {
                continue;
            }
            let Some(node) = state_ref.nodes.get(target.node) else {
                continue;
            };
            if !matches!(node.state, NodeState::Clean | NodeState::Dirty) {
                continue;
            }
            if node.kind != NodeKindTag::Effect {
                scratch
                    .frontier
                    .extend(state_ref.subscriber_edges_of(target.node));
            }
            drop(state_ref);
            if scratch.record_external_owner(target.owner_id) {
                scratch.external_scopes.push(target_scope);
            }
        }

        for scope in &scratch.external_scopes {
            scope
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?;
        }

        for target in scratch.frontier.iter().copied() {
            if target.owner_id == self.owner_id {
                let Some(node) = self.nodes.get_mut(target.node) else {
                    continue;
                };
                if !matches!(node.state, NodeState::Clean | NodeState::Dirty) {
                    continue;
                }
                node.state = NodeState::Check;
                if node.kind == NodeKindTag::Effect && !node.queued {
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
                if node.kind == NodeKindTag::Effect && !node.queued {
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
            }
        }
        Ok(())
    }

    pub(crate) fn is_settled(&self, id: NodeId) -> ReactiveResult<bool> {
        Ok(self
            .nodes
            .get(id)
            .is_some_and(|node| node.state == NodeState::Clean)
            && self
                .scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .global_queue
                .is_empty()
            && self
                .scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .worklist
                .is_empty())
    }
}

pub(crate) fn propagate_pending_sources<'scope>(
    observer_state: &ScopeState<'scope>,
    sources: SmallVec<[TargetNode; 8]>,
) -> ReactiveResult<()> {
    let (scheduler, observer_owner) = {
        let state = observer_state.try_borrow()?;
        (state.scheduler.clone(), state.owner_id)
    };
    for source in sources {
        let source_state = if source.owner_id == observer_owner {
            Some(observer_state.clone())
        } else {
            scheduler
                .try_borrow()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .get_scope_for_edge_cleanup(source.owner_id)?
        };
        if let Some(source_state) = source_state {
            source_state
                .try_borrow_mut()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .queue_dependents(source.node)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::arithmetic_side_effects,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic
    )]

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
                                local_state
                                    .try_borrow()
                                    .expect("state read")
                                    .subscriber_edges_of(local_raw)
                                    .count(),
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
                    .try_borrow()
                    .expect("state read")
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
                        .try_borrow()
                        .expect("state read")
                        .subscriber_edges_of(source_raw)
                        .count(),
                    1
                );
                assert_eq!(
                    effect_state
                        .try_borrow()
                        .expect("state read")
                        .dependency_edges_of(effect_raw)
                        .count(),
                    1
                );

                let _ = dispose_nodes(&source_state, vec![source_raw]);

                assert_eq!(
                    effect_state
                        .try_borrow()
                        .expect("state read")
                        .dependency_edges_of(effect_raw)
                        .count(),
                    0
                );
            });
        });
    }

    #[test]
    fn duplicate_tracking_keeps_bidirectional_hashset_adjacency_unique() {
        let mut runtime = Runtime::new();
        transient(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let source_state = source.handle.state();
            let source_raw = source.handle.raw();
            let effect = scope
                .effect(
                    move || {
                        assert_eq!(source.get(), Ok(0));
                        assert_eq!(source.get(), Ok(0));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            let effect_state = effect.handle.state();
            let effect_raw = effect.handle.raw();
            let (source_target, observer_target) = {
                let source_ref = source_state.try_borrow().expect("source state read");
                let effect_ref = effect_state.try_borrow().expect("effect state read");
                (
                    TargetNode {
                        owner_id: source_ref.owner_id,
                        node: source_raw,
                    },
                    TargetNode {
                        owner_id: effect_ref.owner_id,
                        node: effect_raw,
                    },
                )
            };
            let mut effect_ref = effect_state.try_borrow_mut().expect("effect state write");
            effect_ref.add_dependency(effect_raw, source_target);
            drop(effect_ref);
            let mut source_ref = source_state.try_borrow_mut().expect("source state write");
            source_ref.add_subscriber(source_raw, observer_target);
            assert_eq!(source_ref.subscriber_edges_of(source_raw).count(), 1);
            drop(source_ref);
            assert_eq!(
                effect_state
                    .try_borrow()
                    .expect("effect state read")
                    .dependency_edges_of(effect_raw)
                    .count(),
                1
            );
        });
    }

    #[test]
    fn propagation_scratch_is_reused_and_reset_after_each_notification() {
        let mut runtime = Runtime::new();
        transient(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("source creation");
            let state = source.handle.state();
            state
                .try_borrow_mut()
                .expect("state write")
                .queue_dependents(source.handle.raw())
                .expect("propagation should succeed");
            let state_ref = state.try_borrow().expect("state read");
            assert_eq!(state_ref.propagation_scratch_pool.len(), 1);
            let scratch = state_ref
                .propagation_scratch_pool
                .first()
                .expect("scratch pool entry");
            assert!(scratch.frontier.is_empty());
            assert!(scratch.visited.is_empty());
            assert!(scratch.external_owner_ids.is_empty());
            assert!(scratch.external_scopes.is_empty());
        });
    }

    #[test]
    fn disposal_scratch_is_reused_and_reset_after_each_batch() {
        let mut runtime = Runtime::new();
        transient(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("source creation");
            let state = source.handle.state();
            let raw = source.handle.raw();
            dispose_nodes(&state, vec![raw]).expect("disposal should succeed");

            let state_ref = state.try_borrow().expect("state read");
            assert_eq!(state_ref.disposal_scratch_pool.len(), 1);
            let scratch = state_ref
                .disposal_scratch_pool
                .first()
                .expect("disposal scratch pool entry");
            assert!(scratch.pending.is_empty());
            assert!(scratch.visited.is_empty());
            assert!(scratch.nodes.is_empty());
            assert!(scratch.external_owner_ids.is_empty());
            assert!(scratch.removed_targets.is_empty());
        });
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn scratch_statistics_capture_pool_reuse_and_high_water() {
        let mut runtime = Runtime::new();
        transient(&mut runtime, |scope| {
            let (source, _) = scope.signal(0i32).expect("source creation");
            let state = source.handle.state();
            state
                .try_borrow_mut()
                .expect("state write")
                .queue_dependents(source.handle.raw())
                .expect("first propagation should succeed");
            state
                .try_borrow_mut()
                .expect("state write")
                .queue_dependents(source.handle.raw())
                .expect("second propagation should succeed");

            let (disposable, _) = scope.signal(1i32).expect("disposable creation");
            dispose_nodes(&state, vec![disposable.handle.raw()]).expect("disposal should succeed");

            let snapshot = state
                .try_borrow()
                .expect("state read")
                .runtime_snapshot()
                .expect("runtime snapshot");
            assert_eq!(snapshot.propagation_scratch_pool_hits, 1);
            assert_eq!(snapshot.propagation_scratch_pool_misses, 1);
            assert_eq!(snapshot.propagation_frontier_high_water, 0);
            assert_eq!(snapshot.disposal_scratch_pool_hits, 0);
            assert_eq!(snapshot.disposal_scratch_pool_misses, 1);
            assert_eq!(snapshot.disposal_nodes_high_water, 1);
            assert_eq!(snapshot.disposal_visited_high_water, 1);
            assert_eq!(snapshot.disposal_targets_high_water, 0);
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
                    let child_state_ref = child_state.try_borrow().expect("state read");
                    let source_state_ref = source_state.try_borrow().expect("state read");
                    (child_state_ref.owner_id, source_state_ref.owner_id)
                };
                let source_borrow = source_state.try_borrow_mut().expect("state write");
                let mut child_borrow = child_state.try_borrow_mut().expect("state write");
                let result = child_borrow.clear_dependencies(effect_raw);

                assert_eq!(result, Err(ReactiveError::BorrowConflict));
                assert_eq!(
                    source_borrow
                        .subscriber_edges_of(source_raw)
                        .filter(|target| {
                            *target
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
                        .filter(|target| {
                            *target
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

                let _ = child_state
                    .try_borrow_mut()
                    .expect("state write")
                    .clear_dependencies(effect_raw);
                assert_eq!(
                    source_state
                        .try_borrow()
                        .expect("state read")
                        .subscriber_edges_of(source_raw)
                        .count(),
                    0
                );
                assert_eq!(
                    child_state
                        .try_borrow()
                        .expect("state read")
                        .dependency_edges_of(effect_raw)
                        .count(),
                    0
                );
            });
        });
    }

    #[test]
    fn transaction_commit_conflict_preserves_both_sides_of_the_edge() {
        let scheduler = GlobalScheduler::new();
        let source_storage = ScopeStorage::new(scheduler.clone()).expect("source owner setup");
        let observer_storage = ScopeStorage::new(scheduler).expect("observer owner setup");
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
            owner_id: observer_state.try_borrow().expect("state read").owner_id,
            node: effect_raw,
        };
        let source_target = TargetNode {
            owner_id: source_state.try_borrow().expect("state read").owner_id,
            node: source_raw,
        };

        observer_state
            .try_borrow_mut()
            .expect("state write")
            .begin_dependency_transaction(effect_raw);
        let source_borrow = source_state.try_borrow_mut().expect("state write");
        let mut observer_borrow = observer_state.try_borrow_mut().expect("state write");
        assert_eq!(
            observer_borrow.commit_dependency_transaction(effect_raw),
            Err(ReactiveError::BorrowConflict)
        );
        assert_eq!(
            observer_borrow
                .dependency_edges_of(effect_raw)
                .filter(|target| *target == source_target)
                .count(),
            1
        );
        assert_eq!(
            source_borrow
                .subscriber_edges_of(source_raw)
                .filter(|target| *target == observer_target)
                .count(),
            1
        );
        drop(observer_borrow);
        drop(source_borrow);

        observer_state
            .try_borrow_mut()
            .expect("state write")
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
