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
}

const _: () = assert!(size_of::<NodeCore>() == 32);

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
    pub(crate) postorder: Vec<NodeId>,
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
        for buffer in [&mut self.pending, &mut self.nodes, &mut self.postorder] {
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
/// Each node owns both hash sets directly. This keeps insertion, duplicate
/// checks, and removal independent of the number of neighboring edges without
/// a second edge arena or a duplicated stable edge record.
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

/// Ownership topology for nodes in one reactive scope.
///
/// `roots` and each child vector preserve insertion order. Callers that need
/// the historical child-first cleanup order receive those vectors reversed by
/// [`OwnershipTree::children_of`]. Nodes in `detached` are temporarily outside
/// the indexed tree while their cleanup and payload destruction finish. The
/// private membership index keeps link checks local; full validation compares
/// it with the Vec/SecondaryMap topology.
pub(crate) struct OwnershipTree {
    pub(crate) roots: Vec<NodeId>,
    pub(crate) children: SecondaryMap<NodeId, Vec<NodeId>>,
    pub(crate) detached: HashSet<NodeId>,
    indexed: HashSet<NodeId>,
}

impl OwnershipTree {
    pub(crate) fn new() -> Self {
        Self {
            roots: Vec::new(),
            children: SecondaryMap::new(),
            detached: HashSet::new(),
            indexed: HashSet::new(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.roots.is_empty()
            && self.children.is_empty()
            && self.detached.is_empty()
            && self.indexed.is_empty()
    }

    fn validate_with_pending(
        &self,
        nodes: &SlotMap<NodeId, NodeCore>,
        pending: Option<NodeId>,
    ) -> ReactiveResult<()> {
        let mut indexed = HashSet::new();
        for root in &self.roots {
            if Some(*root) == pending {
                return Err(ReactiveError::InvariantViolation);
            }
            let node = nodes.get(*root).ok_or(ReactiveError::InvariantViolation)?;
            if !node.parent.is_dangling() || self.detached.contains(root) || !indexed.insert(*root)
            {
                return Err(ReactiveError::InvariantViolation);
            }
        }

        for (parent, children) in &self.children {
            nodes.get(parent).ok_or(ReactiveError::InvariantViolation)?;
            if children.is_empty() {
                return Err(ReactiveError::InvariantViolation);
            }
            let mut local = HashSet::new();
            for child in children {
                if Some(*child) == pending {
                    return Err(ReactiveError::InvariantViolation);
                }
                let child_node = nodes.get(*child).ok_or(ReactiveError::InvariantViolation)?;
                if child_node.parent != parent
                    || self.detached.contains(child)
                    || !local.insert(*child)
                    || !indexed.insert(*child)
                {
                    return Err(ReactiveError::InvariantViolation);
                }
            }
        }

        for detached in &self.detached {
            if Some(*detached) == pending {
                return Err(ReactiveError::InvariantViolation);
            }
            nodes
                .get(*detached)
                .ok_or(ReactiveError::InvariantViolation)?;
            if indexed.contains(detached) {
                return Err(ReactiveError::InvariantViolation);
            }
        }

        if indexed != self.indexed {
            return Err(ReactiveError::InvariantViolation);
        }

        if nodes
            .keys()
            .any(|id| Some(id) != pending && !indexed.contains(&id) && !self.detached.contains(&id))
        {
            return Err(ReactiveError::InvariantViolation);
        }
        Ok(())
    }

    pub(crate) fn validate(&self, nodes: &SlotMap<NodeId, NodeCore>) -> ReactiveResult<()> {
        self.validate_with_pending(nodes, None)
    }

    pub(crate) fn roots_snapshot(
        &self,
        nodes: &SlotMap<NodeId, NodeCore>,
    ) -> ReactiveResult<Vec<NodeId>> {
        self.validate(nodes)?;
        Ok(self.roots.clone())
    }

    pub(crate) fn disposal_roots(
        &self,
        nodes: &SlotMap<NodeId, NodeCore>,
    ) -> ReactiveResult<Vec<NodeId>> {
        self.validate(nodes)?;
        let mut roots = self.roots.clone();
        roots.extend(self.detached.iter().copied());
        Ok(roots)
    }

    pub(crate) fn children_of(
        &self,
        nodes: &SlotMap<NodeId, NodeCore>,
        parent: NodeId,
    ) -> ReactiveResult<Vec<NodeId>> {
        if !nodes.contains_key(parent) {
            return Err(ReactiveError::InvariantViolation);
        }
        let Some(children) = self.children.get(parent) else {
            return Ok(Vec::new());
        };
        if children.is_empty() {
            return Err(ReactiveError::InvariantViolation);
        }
        let mut seen = HashSet::new();
        for child in children {
            let child_node = nodes.get(*child).ok_or(ReactiveError::InvariantViolation)?;
            if child_node.parent != parent
                || self.detached.contains(child)
                || !self.indexed.contains(child)
                || !seen.insert(*child)
            {
                return Err(ReactiveError::InvariantViolation);
            }
        }
        let mut result = children.clone();
        result.reverse();
        Ok(result)
    }

    pub(crate) fn link_child(
        &mut self,
        nodes: &SlotMap<NodeId, NodeCore>,
        parent: NodeId,
        child: NodeId,
    ) -> ReactiveResult<()> {
        let child_node = nodes.get(child).ok_or(ReactiveError::InvariantViolation)?;
        let child_is_indexed = self.indexed.contains(&child);
        let parent_is_indexed = parent.is_valid() && self.indexed.contains(&parent);
        if self.detached.contains(&child)
            || (parent.is_valid() && !nodes.contains_key(parent))
            || (parent.is_valid() && self.detached.contains(&parent))
            || (parent.is_valid() && !parent_is_indexed)
            || child_node.parent != parent
            || child_is_indexed
            || self.roots.contains(&child)
            || (parent.is_valid()
                && self
                    .children
                    .get(parent)
                    .is_some_and(|children| children.contains(&child)))
        {
            return Err(ReactiveError::InvariantViolation);
        }
        if !self.indexed.insert(child) {
            return Err(ReactiveError::InvariantViolation);
        }
        if parent.is_dangling() {
            self.roots.push(child);
        } else if let Some(children) = self.children.get_mut(parent) {
            children.push(child);
        } else {
            self.children.insert(parent, vec![child]);
        }
        Ok(())
    }

    pub(crate) fn detach_roots(
        &mut self,
        nodes: &SlotMap<NodeId, NodeCore>,
        roots: &[NodeId],
    ) -> ReactiveResult<()> {
        self.validate(nodes)?;
        let mut requested = HashSet::new();
        for root in roots {
            if !requested.insert(*root) || !nodes.contains_key(*root) {
                return Err(ReactiveError::InvariantViolation);
            }
            if self.detached.contains(root) {
                return Err(ReactiveError::InvariantViolation);
            }
            let node = nodes.get(*root).ok_or(ReactiveError::InvariantViolation)?;
            if node.parent.is_dangling() {
                if !self.roots.contains(root) {
                    return Err(ReactiveError::InvariantViolation);
                }
            } else if !self
                .children
                .get(node.parent)
                .is_some_and(|children| children.contains(root))
            {
                return Err(ReactiveError::InvariantViolation);
            }
        }
        for root in roots {
            let parent = nodes
                .get(*root)
                .ok_or(ReactiveError::InvariantViolation)?
                .parent;
            if parent.is_dangling() {
                remove_once(&mut self.roots, *root)?;
            } else {
                let children = self
                    .children
                    .get_mut(parent)
                    .ok_or(ReactiveError::InvariantViolation)?;
                remove_once(children, *root)?;
                if children.is_empty() {
                    self.children.remove(parent);
                }
            }
            self.indexed.remove(root);
            self.detached.insert(*root);
        }
        self.validate(nodes)
    }

    pub(crate) fn detach_nodes(
        &mut self,
        nodes: &SlotMap<NodeId, NodeCore>,
        roots: &[NodeId],
        subtree: &[NodeId],
    ) -> ReactiveResult<()> {
        self.validate(nodes)?;
        let mut subtree_ids = HashSet::new();
        for id in subtree {
            if !subtree_ids.insert(*id) || !nodes.contains_key(*id) {
                return Err(ReactiveError::InvariantViolation);
            }
        }
        let mut requested = HashSet::new();
        for root in roots {
            if !subtree_ids.contains(root) || !requested.insert(*root) {
                return Err(ReactiveError::InvariantViolation);
            }
        }
        for id in subtree {
            let node = nodes.get(*id).ok_or(ReactiveError::InvariantViolation)?;
            if requested.contains(id) {
                if !self.detached.contains(id)
                    && node.parent.is_valid()
                    && subtree_ids.contains(&node.parent)
                {
                    return Err(ReactiveError::InvariantViolation);
                }
            } else if node.parent.is_dangling() || !subtree_ids.contains(&node.parent) {
                return Err(ReactiveError::InvariantViolation);
            }
        }
        for root in roots {
            if self.detached.contains(root) {
                continue;
            }
            let node = nodes.get(*root).ok_or(ReactiveError::InvariantViolation)?;
            if node.parent.is_dangling() {
                if !self.roots.contains(root) {
                    return Err(ReactiveError::InvariantViolation);
                }
            } else if !self
                .children
                .get(node.parent)
                .is_some_and(|children| children.contains(root))
            {
                return Err(ReactiveError::InvariantViolation);
            }
        }
        for root in roots {
            if self.detached.contains(root) {
                continue;
            }
            let parent = nodes
                .get(*root)
                .ok_or(ReactiveError::InvariantViolation)?
                .parent;
            if parent.is_dangling() {
                remove_once(&mut self.roots, *root)?;
            } else {
                let children = self
                    .children
                    .get_mut(parent)
                    .ok_or(ReactiveError::InvariantViolation)?;
                remove_once(children, *root)?;
                if children.is_empty() {
                    self.children.remove(parent);
                }
            }
        }
        for id in subtree {
            self.children.remove(*id);
            self.indexed.remove(id);
            self.detached.insert(*id);
        }
        self.validate(nodes)
    }

    pub(crate) fn remove_detached(&mut self, id: NodeId) -> ReactiveResult<()> {
        if !self.detached.remove(&id) {
            return Err(ReactiveError::InvariantViolation);
        }
        Ok(())
    }
}

fn remove_once<T: PartialEq>(items: &mut Vec<T>, item: T) -> ReactiveResult<()> {
    let Some(index) = items.iter().position(|entry| *entry == item) else {
        return Err(ReactiveError::InvariantViolation);
    };
    items.remove(index);
    if items.contains(&item) {
        return Err(ReactiveError::InvariantViolation);
    }
    Ok(())
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
    pub(crate) ownership: OwnershipTree,
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
            ownership: OwnershipTree::new(),
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
            roots: self.ownership.roots.len(),
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

    pub(crate) fn roots_snapshot(&self) -> ReactiveResult<Vec<NodeId>> {
        self.ownership.roots_snapshot(&self.nodes)
    }

    pub(crate) fn disposal_roots(&self) -> ReactiveResult<Vec<NodeId>> {
        self.ownership.disposal_roots(&self.nodes)
    }

    pub(crate) fn children_of(&self, parent: NodeId) -> ReactiveResult<Vec<NodeId>> {
        self.ownership.children_of(&self.nodes, parent)
    }

    pub(crate) fn detach_children(&mut self, parent: NodeId) -> ReactiveResult<Vec<NodeId>> {
        let children = self.children_of(parent)?;
        self.ownership.detach_roots(&self.nodes, &children)?;
        Ok(children)
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

    pub(crate) fn link_child(&mut self, parent: NodeId, child: NodeId) -> ReactiveResult<()> {
        self.ownership.link_child(&self.nodes, parent, child)
    }

    pub(crate) fn detach_nodes(
        &mut self,
        roots: &[NodeId],
        subtree: &[NodeId],
    ) -> ReactiveResult<()> {
        self.ownership.detach_nodes(&self.nodes, roots, subtree)
    }

    pub(crate) fn remove_detached(&mut self, id: NodeId) -> ReactiveResult<()> {
        self.ownership.remove_detached(id)
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
        if let Err(error) = self.link_child(parent, id) {
            self.adjacency.remove(id);
            self.data.remove(id);
            self.nodes.remove(id);
            return Err(error);
        }
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
        if !self.nodes.is_empty()
            || !self.data.is_empty()
            || !self.adjacency.is_empty()
            || !self.ownership.is_empty()
            || !self.root_cleanups.is_empty()
            || !self.dependency_transactions.is_empty()
        {
            return Err(ReactiveError::InvariantViolation);
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
            && self.ownership.is_empty()
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
        if !self.is_active()? {
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

    pub(crate) fn create_callback_detached<T: 'scope, E: 'scope>(
        &mut self,
        callback: TypedSlotAllocation<'scope, CallbackThunk<'scope, T, E>>,
    ) -> ReactiveResult<NodeId> {
        self.register_node(
            NodeCore::new(NodeKindTag::Callback, None, NodeState::Clean),
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
        if !self.is_active()? {
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
        if self.is_active()? {
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
        let mode = if self.is_active()? {
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

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

    use super::*;

    fn insert_node(nodes: &mut SlotMap<NodeId, NodeCore>, parent: Option<NodeId>) -> NodeId {
        nodes.insert(NodeCore::new(NodeKindTag::Signal, parent, NodeState::Clean))
    }

    #[test]
    fn topology_rejects_missing_root_and_orphan_node() {
        let mut nodes = SlotMap::with_key();
        let mut tree = OwnershipTree::new();
        tree.roots.push(NodeId::DANGLING);
        assert_eq!(
            tree.roots_snapshot(&nodes),
            Err(ReactiveError::InvariantViolation)
        );

        tree.roots.clear();
        let _orphan = insert_node(&mut nodes, None);
        assert_eq!(
            tree.roots_snapshot(&nodes),
            Err(ReactiveError::InvariantViolation)
        );
    }

    #[test]
    fn topology_rejects_missing_child_duplicate_link_and_wrong_parent() {
        let mut nodes = SlotMap::with_key();
        let mut tree = OwnershipTree::new();
        let first_parent = insert_node(&mut nodes, None);
        tree.link_child(&nodes, NodeId::DANGLING, first_parent)
            .expect("first parent should become a root");
        let second_parent = insert_node(&mut nodes, None);
        tree.link_child(&nodes, NodeId::DANGLING, second_parent)
            .expect("second parent should become a root");

        let child = insert_node(&mut nodes, Some(first_parent));
        assert_eq!(
            tree.link_child(&nodes, second_parent, child),
            Err(ReactiveError::InvariantViolation)
        );
        tree.link_child(&nodes, first_parent, child)
            .expect("child should link to its declared parent");
        assert_eq!(
            tree.link_child(&nodes, first_parent, child),
            Err(ReactiveError::InvariantViolation)
        );

        tree.children.insert(first_parent, vec![NodeId::DANGLING]);
        assert_eq!(
            tree.children_of(&nodes, first_parent),
            Err(ReactiveError::InvariantViolation)
        );
    }

    #[test]
    fn detached_nodes_cannot_be_relinked_and_children_keep_reverse_order() {
        let mut nodes = SlotMap::with_key();
        let mut tree = OwnershipTree::new();
        let parent = insert_node(&mut nodes, None);
        tree.link_child(&nodes, NodeId::DANGLING, parent)
            .expect("parent should become a root");
        let first = insert_node(&mut nodes, Some(parent));
        tree.link_child(&nodes, parent, first)
            .expect("first child should link");
        let second = insert_node(&mut nodes, Some(parent));
        tree.link_child(&nodes, parent, second)
            .expect("second child should link");

        assert_eq!(
            tree.children_of(&nodes, parent)
                .expect("children should be readable"),
            vec![second, first]
        );
        tree.detach_roots(&nodes, &[second])
            .expect("child should become detached");
        assert_eq!(
            tree.link_child(&nodes, parent, second),
            Err(ReactiveError::InvariantViolation)
        );
        assert_eq!(
            tree.children_of(&nodes, parent)
                .expect("remaining child should be readable"),
            vec![first]
        );
    }
}
