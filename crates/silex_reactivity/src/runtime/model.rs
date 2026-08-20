//! Runtime node and scope-state data structures.

use super::{
    scheduler::{GlobalScheduler, OwnerId, TargetNode},
    storage::{
        CallbackThunk, CleanupThunk, ComputationBehavior, ComputationStorage, NodeStorage,
        TypedNodeRef, TypedSlotAllocation,
    },
};
use crate::{
    ReactiveError, ReactiveResult,
    borrow::{BorrowCell, BorrowRef, BorrowRefMut, BorrowSite, SharedCell},
    error::{ErrorHandlerEntry, ErrorHandlerKey},
    handle::NodeKindTag,
    internal::NodeId,
    unsafe_boundary::ScopedPtr,
};
use slotmap::{SecondaryMap, SlotMap};
use smallvec::SmallVec;
use std::{
    collections::{HashMap, HashSet, hash_map, hash_set},
    mem::{size_of, take},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[cfg(feature = "test-support")]
use super::scheduler::{active_observer_for, observer_recovery_failures};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum NodeState {
    Clean = 0,
    Check = 1,
    Dirty = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScopePhase {
    Active,
    Quiescing,
    RunningCleanup,
    Detaching,
    Released,
}

/// Selects the computation tree parent used during node registration.
///
/// Detached computations remain roots of this reactive owner. Their initial
/// execution still installs the computation as `current_owner`, so nodes and
/// cleanups created by the callback retain the usual nested ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComputationParent {
    Current,
    Detached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredAccessMode {
    Active,
    RunningCleanup,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct NodeCore {
    pub(crate) kind: NodeKindTag,
    pub(crate) state: NodeState,
    pub(crate) running: bool,
    pub(crate) queued: bool,
    pub(crate) version: u32,
    pub(crate) updated_epoch: u64,
    pub(crate) last_computed_epoch: u64,
    pub(crate) parent: NodeId,
    pub(crate) first_child: NodeId,
    pub(crate) next_sibling: NodeId,
    pub(crate) prev_sibling: NodeId,
}

const _: () = assert!(size_of::<NodeCore>() == 56);

impl NodeCore {
    pub(crate) fn new(kind: NodeKindTag, parent: Option<NodeId>, state: NodeState) -> Self {
        Self {
            kind,
            state,
            running: false,
            queued: false,
            version: 0,
            updated_epoch: 0,
            last_computed_epoch: 0,
            parent: NodeId::from_option(parent),
            first_child: NodeId::DANGLING,
            next_sibling: NodeId::DANGLING,
            prev_sibling: NodeId::DANGLING,
        }
    }

    pub(crate) fn is_computation(&self) -> bool {
        matches!(self.kind, NodeKindTag::Effect | NodeKindTag::Computed)
    }
}

pub(crate) struct NodeData<'scope> {
    pub(crate) storage: Rc<NodeStorage<'scope>>,
    pub(crate) cleanups: Vec<CleanupThunk<'scope>>,
}

impl<'scope> NodeData<'scope> {
    pub(crate) fn new(storage: Rc<NodeStorage<'scope>>) -> Self {
        Self {
            storage,
            cleanups: Vec::new(),
        }
    }
}

const DEPENDENCY_INLINE_LIMIT: usize = 8;
const BUFFER_RETAIN_LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DependencyOrigin {
    Existing,
    New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ObservedDependency {
    pub(crate) target: TargetNode,
    pub(crate) origin: DependencyOrigin,
}

pub(crate) struct DependencyBuffer {
    inline: SmallVec<[ObservedDependency; DEPENDENCY_INLINE_LIMIT]>,
    overflow: Option<HashMap<TargetNode, DependencyOrigin>>,
}

impl Default for DependencyBuffer {
    fn default() -> Self {
        Self {
            inline: SmallVec::new(),
            overflow: None,
        }
    }
}

pub(crate) enum DependencyBufferIter<'a> {
    Inline(std::slice::Iter<'a, ObservedDependency>),
    Overflow(hash_map::Keys<'a, TargetNode, DependencyOrigin>),
}

impl Iterator for DependencyBufferIter<'_> {
    type Item = TargetNode;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Inline(iter) => iter.next().map(|entry| entry.target),
            Self::Overflow(iter) => iter.next().copied(),
        }
    }
}

impl DependencyBuffer {
    pub(crate) fn insert(&mut self, target: TargetNode, origin: DependencyOrigin) {
        if let Some(overflow) = self.overflow.as_mut() {
            overflow.entry(target).or_insert(origin);
            return;
        }
        if self.inline.iter().any(|entry| entry.target == target) {
            return;
        }
        if self.inline.len() < DEPENDENCY_INLINE_LIMIT {
            self.inline.push(ObservedDependency { target, origin });
            return;
        }
        self.promote();
        if let Some(overflow) = self.overflow.as_mut() {
            overflow.insert(target, origin);
        }
    }

    pub(crate) fn contains(&self, target: TargetNode) -> bool {
        self.overflow.as_ref().map_or_else(
            || self.inline.iter().any(|entry| entry.target == target),
            |overflow| overflow.contains_key(&target),
        )
    }

    pub(crate) fn is_new(&self, target: TargetNode) -> bool {
        self.overflow.as_ref().map_or_else(
            || {
                self.inline
                    .iter()
                    .find(|entry| entry.target == target)
                    .is_some_and(|entry| entry.origin == DependencyOrigin::New)
            },
            |overflow| overflow.get(&target) == Some(&DependencyOrigin::New),
        )
    }

    pub(crate) fn iter(&self) -> DependencyBufferIter<'_> {
        match self.overflow.as_ref() {
            Some(overflow) => DependencyBufferIter::Overflow(overflow.keys()),
            None => DependencyBufferIter::Inline(self.inline.iter()),
        }
    }

    fn promote(&mut self) {
        let entries = take(&mut self.inline);
        let mut overflow = HashMap::new();
        for entry in entries {
            overflow.insert(entry.target, entry.origin);
        }
        self.overflow = Some(overflow);
    }

    pub(crate) fn reset(&mut self) {
        self.inline.clear();
        if self
            .overflow
            .as_ref()
            .is_some_and(|overflow| overflow.len() > BUFFER_RETAIN_LIMIT)
        {
            self.overflow = None;
        } else if let Some(overflow) = self.overflow.as_mut() {
            overflow.clear();
        }
    }
}

pub(crate) struct TargetBuffer {
    inline: SmallVec<[TargetNode; DEPENDENCY_INLINE_LIMIT]>,
    overflow: Option<HashSet<TargetNode>>,
}

impl Default for TargetBuffer {
    fn default() -> Self {
        Self {
            inline: SmallVec::new(),
            overflow: None,
        }
    }
}

impl TargetBuffer {
    pub(crate) fn insert(&mut self, target: TargetNode) {
        if let Some(overflow) = self.overflow.as_mut() {
            overflow.insert(target);
            return;
        }
        if self.inline.contains(&target) {
            return;
        }
        if self.inline.len() < DEPENDENCY_INLINE_LIMIT {
            self.inline.push(target);
            return;
        }
        let entries = take(&mut self.inline);
        let mut overflow = HashSet::new();
        overflow.extend(entries);
        overflow.insert(target);
        self.overflow = Some(overflow);
    }

    pub(crate) fn take_targets(&mut self) -> SmallVec<[TargetNode; DEPENDENCY_INLINE_LIMIT]> {
        if let Some(overflow) = self.overflow.take() {
            overflow.into_iter().collect()
        } else {
            take(&mut self.inline)
        }
    }

    pub(crate) fn reset(&mut self) {
        self.inline.clear();
        if self
            .overflow
            .as_ref()
            .is_some_and(|overflow| overflow.len() > BUFFER_RETAIN_LIMIT)
        {
            self.overflow = None;
        } else if let Some(overflow) = self.overflow.as_mut() {
            overflow.clear();
        }
    }
}

pub(crate) struct PropagationScratch<'scope> {
    pub(crate) frontier: Vec<TargetNode>,
    pub(crate) visited: HashSet<TargetNode>,
    pub(crate) external_owner_ids: SmallVec<[OwnerId; 8]>,
    external_owner_set: Option<HashSet<OwnerId>>,
    pub(crate) external_scopes: Vec<ScopeState<'scope>>,
    #[cfg(feature = "test-support")]
    external_owner_promotions: usize,
}

impl<'scope> Default for PropagationScratch<'scope> {
    fn default() -> Self {
        Self {
            frontier: Vec::new(),
            visited: HashSet::new(),
            external_owner_ids: SmallVec::new(),
            external_owner_set: None,
            external_scopes: Vec::new(),
            #[cfg(feature = "test-support")]
            external_owner_promotions: 0,
        }
    }
}

impl<'scope> PropagationScratch<'scope> {
    pub(crate) fn record_external_owner(&mut self, owner_id: OwnerId) -> bool {
        if let Some(owner_set) = self.external_owner_set.as_mut() {
            return owner_set.insert(owner_id);
        }
        if self.external_owner_ids.contains(&owner_id) {
            return false;
        }
        if self.external_owner_ids.len() < 8 {
            self.external_owner_ids.push(owner_id);
            return true;
        }
        let mut owner_set = HashSet::new();
        owner_set.extend(self.external_owner_ids.iter().copied());
        owner_set.insert(owner_id);
        self.external_owner_set = Some(owner_set);
        #[cfg(feature = "test-support")]
        {
            self.external_owner_promotions = self.external_owner_promotions.saturating_add(1);
        }
        true
    }

    pub(crate) fn reset(&mut self) {
        self.frontier.clear();
        self.visited.clear();
        self.external_owner_ids.clear();
        if self
            .external_owner_set
            .as_ref()
            .is_some_and(|owner_set| owner_set.len() > BUFFER_RETAIN_LIMIT)
        {
            self.external_owner_set = None;
        } else if let Some(owner_set) = self.external_owner_set.as_mut() {
            owner_set.clear();
        }
        self.external_scopes.clear();
        #[cfg(feature = "test-support")]
        {
            self.external_owner_promotions = 0;
        }
    }
}

#[derive(Default)]
pub(crate) struct DisposalScratch {
    pub(crate) pending: Vec<NodeId>,
    pub(crate) visited: HashSet<NodeId>,
    pub(crate) nodes: Vec<NodeId>,
    pub(crate) external_owner_ids: HashSet<OwnerId>,
    pub(crate) removed_targets: Vec<TargetNode>,
}

#[cfg(feature = "test-support")]
#[derive(Default)]
pub(crate) struct ScratchStats {
    pub(crate) propagation_pool_hits: usize,
    pub(crate) propagation_pool_misses: usize,
    pub(crate) propagation_frontier_high_water: usize,
    pub(crate) propagation_visited_high_water: usize,
    pub(crate) propagation_external_owner_promotions: usize,
    pub(crate) disposal_pool_hits: usize,
    pub(crate) disposal_pool_misses: usize,
    pub(crate) disposal_nodes_high_water: usize,
    pub(crate) disposal_visited_high_water: usize,
    pub(crate) disposal_targets_high_water: usize,
}

impl DisposalScratch {
    pub(crate) fn reset(&mut self) {
        for buffer in [&mut self.pending, &mut self.nodes] {
            if buffer.capacity() > BUFFER_RETAIN_LIMIT {
                *buffer = Vec::new();
            } else {
                buffer.clear();
            }
        }
        if self.removed_targets.capacity() > BUFFER_RETAIN_LIMIT {
            self.removed_targets = Vec::new();
        } else {
            self.removed_targets.clear();
        }
        if self.visited.capacity() > BUFFER_RETAIN_LIMIT {
            self.visited = HashSet::new();
        } else {
            self.visited.clear();
        }
        if self.external_owner_ids.capacity() > BUFFER_RETAIN_LIMIT {
            self.external_owner_ids = HashSet::new();
        } else {
            self.external_owner_ids.clear();
        }
    }
}

pub(crate) struct DependencyTransaction {
    pub(crate) observer: NodeId,
    pub(crate) current: DependencyBuffer,
    pub(crate) pending_sources: TargetBuffer,
}

/// Direct indexes for the two directions of a node's reactive edges.
///
/// The edge arena remains the owner of stable edge ids, while these maps are
/// the hot-path adjacency structure. This keeps insertion, duplicate checks,
/// and removal independent of the number of neighboring edges.
pub(crate) struct NodeAdjacency {
    pub(crate) subscribers: HashSet<TargetNode>,
    pub(crate) dependencies: HashSet<TargetNode>,
}

impl NodeAdjacency {
    pub(crate) fn new() -> Self {
        Self {
            subscribers: HashSet::new(),
            dependencies: HashSet::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct RootLink {
    previous: NodeId,
    next: NodeId,
}

/// Ordered root index with constant-time removal.
///
/// Roots are not part of a node's child list, so keeping a separate index
/// avoids shifting or retaining the complete root collection during disposal.
#[derive(Clone)]
pub(crate) struct RootSet {
    links: HashMap<NodeId, RootLink>,
    first: NodeId,
    last: NodeId,
}

impl RootSet {
    pub(crate) fn new() -> Self {
        Self {
            links: HashMap::new(),
            first: NodeId::DANGLING,
            last: NodeId::DANGLING,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    #[cfg(feature = "test-support")]
    pub(crate) fn len(&self) -> usize {
        self.links.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    pub(crate) fn push(&mut self, root: NodeId) {
        let previous = self.last;
        self.links.insert(
            root,
            RootLink {
                previous,
                next: NodeId::DANGLING,
            },
        );
        if previous.is_dangling() {
            self.first = root;
        } else if let Some(previous_link) = self.links.get_mut(&previous) {
            previous_link.next = root;
        }
        self.last = root;
    }

    pub(crate) fn remove(&mut self, root: NodeId) {
        let Some(link) = self.links.remove(&root) else {
            return;
        };
        if link.previous.is_dangling() {
            self.first = link.next;
        } else if let Some(previous) = self.links.get_mut(&link.previous) {
            previous.next = link.next;
        }
        if link.next.is_dangling() {
            self.last = link.previous;
        } else if let Some(next) = self.links.get_mut(&link.next) {
            next.previous = link.previous;
        }
    }

    pub(crate) fn to_vec(&self) -> Vec<NodeId> {
        let mut roots = Vec::with_capacity(self.links.len());
        let mut current = self.first;
        while current.is_valid() {
            roots.push(current);
            current = self
                .links
                .get(&current)
                .map(|link| link.next)
                .unwrap_or(NodeId::DANGLING);
        }
        roots
    }
}

impl IntoIterator for RootSet {
    type Item = NodeId;
    type IntoIter = std::vec::IntoIter<NodeId>;

    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}

/// Iterator over child nodes in an intra-arena sibling chain.
pub(crate) struct ChildrenIter<'a, 'scope> {
    state: &'a ScopeStateInner<'scope>,
    curr: NodeId,
}

impl Iterator for ChildrenIter<'_, '_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_dangling() {
            return None;
        }
        let item = self.curr;
        self.curr = self
            .state
            .nodes
            .get(item)
            .map(|n| n.next_sibling)
            .unwrap_or(NodeId::DANGLING);
        Some(item)
    }
}

/// Iterator over entries in a node's direct edge index.
pub(crate) struct TargetIter<'a> {
    inner: Option<hash_set::Iter<'a, TargetNode>>,
}

impl Iterator for TargetIter<'_> {
    type Item = TargetNode;

    fn next(&mut self) -> Option<Self::Item> {
        Some(*self.inner.as_mut()?.next()?)
    }
}

/// Reactive graph nodes, scheduling state, and stable storage owned by one lexical scope.
pub(crate) struct ScopeStateInner<'scope> {
    pub(crate) owner_id: OwnerId,
    pub(crate) scheduler: SharedCell<GlobalScheduler>,
    pub(crate) phase: ScopePhase,
    pub(crate) nodes: SlotMap<NodeId, NodeCore>,
    pub(crate) data: SecondaryMap<NodeId, NodeData<'scope>>,
    pub(crate) adjacency: SecondaryMap<NodeId, NodeAdjacency>,
    pub(crate) roots: RootSet,
    pub(crate) current_owner: Option<NodeId>,
    pub(crate) root_cleanups: Vec<CleanupThunk<'scope>>,
    pub(crate) dependency_transactions: Vec<DependencyTransaction>,
    pub(crate) dependency_buffer_pool: Vec<DependencyBuffer>,
    pub(crate) target_buffer_pool: Vec<TargetBuffer>,
    pub(crate) propagation_scratch_pool: Vec<PropagationScratch<'scope>>,
    pub(crate) disposal_scratch_pool: Vec<DisposalScratch>,
    #[cfg(feature = "test-support")]
    pub(crate) scratch_stats: ScratchStats,
    pub(crate) error_handlers: SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'scope>>,
    pub(crate) pending_error_handlers: Vec<(ErrorHandlerKey, ScopedPtr<()>)>,
}

/// A reference-counted wrapper around the inner scope state.
#[derive(Clone)]
pub(crate) struct ScopeState<'scope> {
    pub(crate) inner: SharedCell<ScopeStateInner<'scope>>,
}

impl<'scope> ScopeState<'scope> {
    pub(crate) fn new(owner_id: OwnerId, scheduler: SharedCell<GlobalScheduler>) -> Self {
        Self {
            inner: Rc::new(BorrowCell::new(
                ScopeStateInner::new(owner_id, scheduler),
                BorrowSite::ScopeState,
            )),
        }
    }

    pub(crate) fn from_inner(inner: SharedCell<ScopeStateInner<'scope>>) -> Self {
        Self { inner }
    }

    pub(crate) fn into_inner(self) -> SharedCell<ScopeStateInner<'scope>> {
        self.inner
    }

    pub(crate) fn inner(&self) -> &SharedCell<ScopeStateInner<'scope>> {
        &self.inner
    }

    pub(crate) fn try_borrow(
        &self,
    ) -> Result<BorrowRef<'_, ScopeStateInner<'scope>>, ReactiveError> {
        self.inner
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)
    }

    pub(crate) fn try_borrow_mut(
        &self,
    ) -> Result<BorrowRefMut<'_, ScopeStateInner<'scope>>, ReactiveError> {
        self.inner
            .try_write()
            .map_err(|_| ReactiveError::BorrowConflict)
    }

    pub(crate) fn drop_error_handlers(
        handlers: SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'_>>,
    ) -> Vec<Box<dyn std::any::Any + Send>> {
        ScopeStateInner::drop_error_handlers(handlers)
    }

    pub(crate) fn validate_callback_endpoint(&self, id: NodeId) -> ReactiveResult<()> {
        self.try_borrow()?.validate_callback_endpoint(id)
    }
}

#[cfg(feature = "test-support")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeSnapshot {
    pub nodes: usize,
    pub data: usize,
    pub edges: usize,
    pub roots: usize,
    pub cleanups: usize,
    pub handlers: usize,
    pub queue: usize,
    pub epoch: u64,
    pub observer: bool,
    pub running_queue: bool,
    pub active_owners: usize,
    pub closing_owners: usize,
    pub owner_generation: u64,
    pub active_leases: usize,
    pub queue_recovery: bool,
    pub retained_children: usize,
    pub live_typed_slots: usize,
    pub live_error_slots: usize,
    pub unhandled_close_errors: usize,
    pub dropped_close_reports: usize,
    pub observer_recovery_failures: usize,
    pub propagation_scratch_pool_hits: usize,
    pub propagation_scratch_pool_misses: usize,
    pub propagation_frontier_high_water: usize,
    pub propagation_visited_high_water: usize,
    pub propagation_external_owner_promotions: usize,
    pub disposal_scratch_pool_hits: usize,
    pub disposal_scratch_pool_misses: usize,
    pub disposal_nodes_high_water: usize,
    pub disposal_visited_high_water: usize,
    pub disposal_targets_high_water: usize,
}

impl<'scope> ScopeStateInner<'scope> {
    pub(crate) fn new(owner_id: OwnerId, scheduler: SharedCell<GlobalScheduler>) -> Self {
        Self {
            owner_id,
            scheduler,
            phase: ScopePhase::Active,
            nodes: SlotMap::with_key(),
            data: SecondaryMap::new(),
            adjacency: SecondaryMap::new(),
            roots: RootSet::new(),
            current_owner: None,
            root_cleanups: Vec::new(),
            dependency_transactions: Vec::new(),
            dependency_buffer_pool: Vec::new(),
            target_buffer_pool: Vec::new(),
            propagation_scratch_pool: Vec::new(),
            disposal_scratch_pool: Vec::new(),
            #[cfg(feature = "test-support")]
            scratch_stats: ScratchStats::default(),
            error_handlers: SlotMap::with_key(),
            pending_error_handlers: Vec::new(),
        }
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn record_propagation_scratch(&mut self, scratch: &PropagationScratch<'_>) {
        self.scratch_stats.propagation_frontier_high_water = self
            .scratch_stats
            .propagation_frontier_high_water
            .max(scratch.frontier.len());
        self.scratch_stats.propagation_visited_high_water = self
            .scratch_stats
            .propagation_visited_high_water
            .max(scratch.visited.len());
        self.scratch_stats.propagation_external_owner_promotions = self
            .scratch_stats
            .propagation_external_owner_promotions
            .saturating_add(scratch.external_owner_promotions);
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn record_disposal_scratch(&mut self, scratch: &DisposalScratch) {
        self.scratch_stats.disposal_nodes_high_water = self
            .scratch_stats
            .disposal_nodes_high_water
            .max(scratch.nodes.len());
        self.scratch_stats.disposal_visited_high_water = self
            .scratch_stats
            .disposal_visited_high_water
            .max(scratch.visited.len());
        self.scratch_stats.disposal_targets_high_water = self
            .scratch_stats
            .disposal_targets_high_water
            .max(scratch.removed_targets.len());
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn runtime_snapshot(&self) -> ReactiveResult<RuntimeSnapshot> {
        let cleanups = self.root_cleanups.len().saturating_add(
            self.data
                .values()
                .map(|data| data.cleanups.len())
                .sum::<usize>(),
        );
        let scheduler = self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let active_owners = scheduler.active_owner_ids().len();
        let mut closing_owners: usize = 0;
        for id in scheduler.active_owner_ids() {
            if let Some(state) = scheduler.get_scope_for_edge_cleanup(id)?
                && state.try_borrow()?.phase != ScopePhase::Active
            {
                closing_owners = closing_owners.saturating_add(1);
            }
        }
        Ok(RuntimeSnapshot {
            nodes: self.nodes.len(),
            data: self.data.len(),
            edges: self
                .adjacency
                .values()
                .map(|adjacency| {
                    adjacency
                        .subscribers
                        .len()
                        .saturating_add(adjacency.dependencies.len())
                })
                .sum(),
            roots: self.roots.len(),
            cleanups,
            handlers: self
                .error_handlers
                .values()
                .filter(|entry| entry.owner.is_active())
                .count(),
            queue: scheduler
                .global_queue
                .len()
                .saturating_add(scheduler.worklist.len()),
            epoch: scheduler.current_epoch(),
            observer: active_observer_for(&self.scheduler)?.is_some(),
            running_queue: scheduler.running_queue,
            active_owners,
            closing_owners,
            owner_generation: self.owner_id.1,
            active_leases: scheduler.active_leases,
            queue_recovery: !scheduler.running_queue
                && scheduler.global_queue.is_empty()
                && scheduler.worklist.is_empty(),
            retained_children: 0,
            live_typed_slots: 0,
            live_error_slots: 0,
            unhandled_close_errors: scheduler.close_reports.len()?,
            dropped_close_reports: scheduler.dropped_close_reports(),
            observer_recovery_failures: observer_recovery_failures(),
            propagation_scratch_pool_hits: self.scratch_stats.propagation_pool_hits,
            propagation_scratch_pool_misses: self.scratch_stats.propagation_pool_misses,
            propagation_frontier_high_water: self.scratch_stats.propagation_frontier_high_water,
            propagation_visited_high_water: self.scratch_stats.propagation_visited_high_water,
            propagation_external_owner_promotions: self
                .scratch_stats
                .propagation_external_owner_promotions,
            disposal_scratch_pool_hits: self.scratch_stats.disposal_pool_hits,
            disposal_scratch_pool_misses: self.scratch_stats.disposal_pool_misses,
            disposal_nodes_high_water: self.scratch_stats.disposal_nodes_high_water,
            disposal_visited_high_water: self.scratch_stats.disposal_visited_high_water,
            disposal_targets_high_water: self.scratch_stats.disposal_targets_high_water,
        })
    }

    pub(crate) fn parent_for_new_node(&self) -> Option<NodeId> {
        self.current_owner
    }

    #[inline]
    pub(crate) fn children_of_head(&self, head: NodeId) -> ChildrenIter<'_, 'scope> {
        ChildrenIter {
            state: self,
            curr: head,
        }
    }

    #[inline]
    pub(crate) fn subscriber_edges_of(&self, node_id: NodeId) -> TargetIter<'_> {
        TargetIter {
            inner: self
                .adjacency
                .get(node_id)
                .map(|adjacency| adjacency.subscribers.iter()),
        }
    }

    #[inline]
    pub(crate) fn dependency_edges_of(&self, node_id: NodeId) -> TargetIter<'_> {
        TargetIter {
            inner: self
                .adjacency
                .get(node_id)
                .map(|adjacency| adjacency.dependencies.iter()),
        }
    }

    pub(crate) fn link_child(&mut self, parent: NodeId, child: NodeId) {
        if parent.is_dangling() {
            self.roots.push(child);
            return;
        }
        let old_first = self
            .nodes
            .get(parent)
            .map(|p| p.first_child)
            .unwrap_or(NodeId::DANGLING);
        if let Some(child_node) = self.nodes.get_mut(child) {
            child_node.next_sibling = old_first;
            child_node.prev_sibling = NodeId::DANGLING;
        }
        if old_first.is_valid()
            && let Some(old_first_node) = self.nodes.get_mut(old_first)
        {
            old_first_node.prev_sibling = child;
        }
        if let Some(parent_node) = self.nodes.get_mut(parent) {
            parent_node.first_child = child;
        }
    }

    pub(crate) fn unlink_child(&mut self, parent: NodeId, child: NodeId) {
        if parent.is_dangling() {
            self.roots.remove(child);
            return;
        }
        if !self.nodes.contains_key(parent) {
            return;
        }
        let Some(child_node) = self.nodes.get(child).copied() else {
            return;
        };
        if child_node.prev_sibling.is_dangling() {
            if let Some(parent_node) = self.nodes.get_mut(parent) {
                parent_node.first_child = child_node.next_sibling;
            }
        } else if let Some(previous) = self.nodes.get_mut(child_node.prev_sibling) {
            previous.next_sibling = child_node.next_sibling;
        }
        if child_node.next_sibling.is_valid()
            && let Some(next) = self.nodes.get_mut(child_node.next_sibling)
        {
            next.prev_sibling = child_node.prev_sibling;
        }
    }

    /// Unified node registration kernel for every owner-local node kind.
    pub(crate) fn register_node(
        &mut self,
        node: NodeCore,
        make_data: impl FnOnce() -> ReactiveResult<NodeData<'scope>>,
    ) -> ReactiveResult<NodeId> {
        self.ensure_active()?;
        let parent = node.parent;
        let data = make_data()?;
        let id = self.nodes.insert(node);
        self.data.insert(id, data);
        self.adjacency.insert(id, NodeAdjacency::new());
        self.link_child(parent, id);
        Ok(id)
    }

    pub(crate) fn is_active(&self) -> ReactiveResult<bool> {
        if self.phase != ScopePhase::Active {
            return Ok(false);
        }
        Ok(self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .is_scope_active(self.owner_id))
    }

    pub(crate) fn try_is_active(&self) -> ReactiveResult<bool> {
        if self.phase != ScopePhase::Active {
            return Ok(false);
        }
        Ok(self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .is_scope_active(self.owner_id))
    }

    pub(crate) fn begin_quiescing(&mut self) -> ReactiveResult<bool> {
        match self.phase {
            ScopePhase::Active => {
                self.phase = ScopePhase::Quiescing;
                Ok(true)
            }
            ScopePhase::Released => Ok(false),
            ScopePhase::Quiescing | ScopePhase::RunningCleanup | ScopePhase::Detaching => Ok(true),
        }
    }

    pub(crate) fn begin_cleanup(&mut self) -> ReactiveResult<()> {
        match self.phase {
            ScopePhase::Quiescing => {
                self.phase = ScopePhase::RunningCleanup;
                Ok(())
            }
            ScopePhase::RunningCleanup | ScopePhase::Detaching => Ok(()),
            ScopePhase::Active => Err(ReactiveError::Reentrant),
            ScopePhase::Released => Err(ReactiveError::NoSuchNode),
        }
    }

    pub(crate) fn begin_detaching(&mut self) -> ReactiveResult<()> {
        match self.phase {
            ScopePhase::RunningCleanup => {
                self.phase = ScopePhase::Detaching;
                Ok(())
            }
            ScopePhase::Detaching => Ok(()),
            ScopePhase::Active | ScopePhase::Quiescing => Err(ReactiveError::Reentrant),
            ScopePhase::Released => Err(ReactiveError::NoSuchNode),
        }
    }

    pub(crate) fn finish_dispose(&mut self) -> ReactiveResult<()> {
        if self.phase != ScopePhase::Detaching {
            return Err(ReactiveError::Reentrant);
        }
        self.current_owner = None;
        self.phase = ScopePhase::Released;
        Ok(())
    }

    pub(crate) fn ready_for_scope_release(&self) -> bool {
        self.phase == ScopePhase::Released
            && self.nodes.is_empty()
            && self.data.is_empty()
            && self.adjacency.values().all(|adjacency| {
                adjacency.subscribers.is_empty() && adjacency.dependencies.is_empty()
            })
            && self.roots.is_empty()
            && self.root_cleanups.is_empty()
            && self.dependency_transactions.is_empty()
    }

    pub(crate) fn allows_final_cleanup_stored_access(&self) -> bool {
        self.phase == ScopePhase::RunningCleanup
    }

    pub(crate) fn node_exists(&self, id: NodeId) -> bool {
        self.nodes.get(id).is_some()
    }

    pub(crate) fn typed_node_ref<T>(&self, id: NodeId) -> ReactiveResult<TypedNodeRef<'scope, T>> {
        self.ensure_active()?;
        let data = self.data.get(id).ok_or(ReactiveError::NoSuchNode)?;
        let identity = data
            .storage
            .payload_identity()
            .ok_or(ReactiveError::NoSuchNode)?;
        Ok(TypedNodeRef::from_pointer(identity))
    }

    pub(crate) fn mark_notified(&mut self, id: NodeId) -> ReactiveResult<bool> {
        if !self.try_is_active()? {
            return Ok(false);
        }
        let epoch = self
            .scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .next_epoch();
        let Some(node) = self.nodes.get_mut(id) else {
            return Ok(false);
        };
        node.updated_epoch = epoch;
        node.version = node.version.wrapping_add(1);
        Ok(true)
    }

    pub(crate) fn validate_node_kind(
        &self,
        id: NodeId,
        expected: NodeKindTag,
    ) -> ReactiveResult<NodeCore> {
        self.ensure_active()?;
        let node = self
            .nodes
            .get(id)
            .copied()
            .ok_or(ReactiveError::NoSuchNode)?;
        if node.kind != expected {
            return Err(ReactiveError::WrongKind);
        }
        Ok(node)
    }

    pub(crate) fn set_ctx(&mut self, owner: Option<NodeId>) {
        self.current_owner = owner;
    }

    pub(crate) fn create_signal<T: 'scope>(
        &mut self,
        value: TypedSlotAllocation<'scope, T>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        let epoch = self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .current_epoch();
        let mut node = NodeCore::new(NodeKindTag::Signal, parent, NodeState::Clean);
        node.updated_epoch = epoch;
        node.last_computed_epoch = epoch;
        self.register_node(node, move || {
            Ok(NodeData::new(Rc::new(NodeStorage::value(value)?)))
        })
    }

    pub(super) fn register_computation(
        &mut self,
        kind: NodeKindTag,
        callback: Box<dyn ComputationBehavior<'scope> + 'scope>,
        parent_strategy: ComputationParent,
    ) -> ReactiveResult<NodeId> {
        let parent = match parent_strategy {
            ComputationParent::Current => self.parent_for_new_node(),
            ComputationParent::Detached => None,
        };
        self.register_node(NodeCore::new(kind, parent, NodeState::Dirty), move || {
            Ok(NodeData::new(Rc::new(NodeStorage::Computation(
                ComputationStorage::new(callback)?,
            ))))
        })
    }

    pub(crate) fn create_stored<T: 'scope>(
        &mut self,
        value: TypedSlotAllocation<'scope, T>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        self.register_node(
            NodeCore::new(NodeKindTag::Stored, parent, NodeState::Clean),
            move || Ok(NodeData::new(Rc::new(NodeStorage::value(value)?))),
        )
    }

    pub(crate) fn create_callback<T: 'scope, E: 'scope>(
        &mut self,
        callback: TypedSlotAllocation<'scope, CallbackThunk<'scope, T, E>>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        self.register_node(
            NodeCore::new(NodeKindTag::Callback, parent, NodeState::Clean),
            move || Ok(NodeData::new(Rc::new(NodeStorage::callback(callback)?))),
        )
    }

    pub(crate) fn create_node_ref<T: 'scope>(
        &mut self,
        value: TypedSlotAllocation<'scope, Option<T>>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        self.register_node(
            NodeCore::new(NodeKindTag::NodeRef, parent, NodeState::Clean),
            move || Ok(NodeData::new(Rc::new(NodeStorage::value(value)?))),
        )
    }

    pub(crate) fn register_cleanup(&mut self, cleanup: CleanupThunk<'scope>) {
        if let Some(owner) = self.current_owner
            && let Some(data) = self.data.get_mut(owner)
        {
            data.cleanups.push(cleanup);
            return;
        }
        self.root_cleanups.push(cleanup);
    }

    pub(crate) fn register_error_handler(
        &mut self,
        entry: ErrorHandlerEntry<'scope>,
    ) -> ReactiveResult<ErrorHandlerKey> {
        self.sweep_error_handlers();
        if !self.try_is_active()? {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(self.error_handlers.insert(entry))
    }

    pub(crate) fn remove_error_handler(&mut self, key: ErrorHandlerKey, identity: ScopedPtr<()>) {
        if self
            .error_handlers
            .get(key)
            .is_some_and(|entry| entry.identity == identity)
        {
            self.error_handlers.remove(key);
        }
    }

    pub(crate) fn sweep_error_handlers(&mut self) {
        self.pending_error_handlers
            .extend(self.error_handlers.iter().filter_map(|(key, entry)| {
                (!entry.owner.is_active() || entry.owner.is_pending_retire())
                    .then_some((key, entry.identity))
            }));
        for (key, identity) in take(&mut self.pending_error_handlers) {
            if self
                .error_handlers
                .get(key)
                .is_some_and(|entry| entry.identity == identity)
            {
                self.error_handlers.remove(key);
            }
        }
    }

    pub(crate) fn take_error_handlers(
        &mut self,
    ) -> SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'scope>> {
        take(&mut self.error_handlers)
    }

    pub(crate) fn drop_error_handlers(
        handlers: SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'scope>>,
    ) -> Vec<Box<dyn std::any::Any + Send>> {
        let mut panics = Vec::new();
        for (_, entry) in handlers {
            if let Err(panic) = catch_unwind(AssertUnwindSafe(|| entry.owner.force_retire())) {
                panics.push(panic);
            }
        }
        panics
    }

    pub(crate) fn has_value(&self, id: NodeId) -> ReactiveResult<bool> {
        let Some(node) = self.nodes.get(id) else {
            return Ok(false);
        };
        match node.kind {
            NodeKindTag::Signal => Ok(true),
            NodeKindTag::Computed => {
                let Some(data) = self.data.get(id) else {
                    return Ok(false);
                };
                let NodeStorage::Computation(storage) = data.storage.as_ref() else {
                    return Ok(false);
                };
                let value = storage
                    .computation
                    .try_peek(|behavior| match behavior.as_ref() {
                        Some(behavior) => behavior.has_value(),
                        None => Ok(false),
                    })?;
                Ok(value.ok_or(ReactiveError::InvariantViolation)??)
            }
            _ => Ok(false),
        }
    }

    fn ensure_active(&self) -> Result<(), ReactiveError> {
        if self.try_is_active()? {
            Ok(())
        } else {
            Err(ReactiveError::NoSuchNode)
        }
    }

    pub(crate) fn value_storage(
        &self,
        id: NodeId,
        reactive: bool,
    ) -> ReactiveResult<Rc<NodeStorage<'scope>>> {
        self.ensure_active()?;
        let node = self.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        let valid_kind = if reactive {
            matches!(node.kind, NodeKindTag::Signal | NodeKindTag::Computed)
        } else {
            node.kind == NodeKindTag::Signal
        };
        if !valid_kind {
            return Err(ReactiveError::WrongKind);
        }
        let data = self.data.get(id).ok_or(ReactiveError::NoSuchNode)?;
        let valid_storage = match node.kind {
            NodeKindTag::Signal => matches!(data.storage.as_ref(), NodeStorage::Value(_)),
            NodeKindTag::Computed => {
                matches!(data.storage.as_ref(), NodeStorage::Computation(_))
            }
            _ => false,
        };
        if !valid_storage {
            return Err(ReactiveError::WrongKind);
        }
        Ok(data.storage.clone())
    }

    pub(crate) fn stored_value_storage(
        &self,
        id: NodeId,
    ) -> ReactiveResult<(Rc<NodeStorage<'scope>>, StoredAccessMode)> {
        let mode = if self.try_is_active()? {
            StoredAccessMode::Active
        } else if self.allows_final_cleanup_stored_access() {
            StoredAccessMode::RunningCleanup
        } else {
            return Err(ReactiveError::NoSuchNode);
        };
        let node = self.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if node.kind != NodeKindTag::Stored {
            return Err(ReactiveError::WrongKind);
        }
        let data = self.data.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if !matches!(data.storage.as_ref(), NodeStorage::Value(_)) {
            return Err(ReactiveError::WrongKind);
        }
        Ok((data.storage.clone(), mode))
    }

    pub(crate) fn node_ref_storage(&self, id: NodeId) -> ReactiveResult<Rc<NodeStorage<'scope>>> {
        self.ensure_active()?;
        let node = self.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if node.kind != NodeKindTag::NodeRef {
            return Err(ReactiveError::WrongKind);
        }
        let data = self.data.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if !matches!(data.storage.as_ref(), NodeStorage::Value(_)) {
            return Err(ReactiveError::WrongKind);
        }
        Ok(data.storage.clone())
    }

    pub(crate) fn callback_storage(&self, id: NodeId) -> ReactiveResult<Rc<NodeStorage<'scope>>> {
        self.validate_node_kind(id, NodeKindTag::Callback)?;
        let data = self.data.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if !matches!(data.storage.as_ref(), NodeStorage::Callback(_)) {
            return Err(ReactiveError::WrongKind);
        }
        Ok(data.storage.clone())
    }

    /// Validate a callback endpoint before an asynchronous typed restore.
    ///
    /// This is deliberately kept beside the normal callback storage lookup so
    /// completion endpoints cannot bypass the node generation and kind checks
    /// used by ordinary callback handles.
    pub(crate) fn validate_callback_endpoint(&self, id: NodeId) -> ReactiveResult<()> {
        let _ = self.callback_storage(id)?;
        Ok(())
    }
}
