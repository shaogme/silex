//! Runtime node and scope-state data structures.

use super::{
    scheduler::{GlobalScheduler, OwnerId, TargetNode},
    storage::{
        CallbackThunk, CleanupThunk, ComputationBehavior, ComputationStorage, NodeStorage,
        TypedNodeRef,
    },
};
use crate::{
    ReactiveError, ReactiveResult,
    error::{ErrorHandlerEntry, ErrorHandlerKey},
    handle::NodeKindTag,
    internal::NodeId,
};
use slotmap::{SecondaryMap, SlotMap};
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::{HashMap, HashSet, hash_map},
    mem::{size_of, take},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[cfg(feature = "test-support")]
use super::scheduler::active_observer_for;

slotmap::new_key_type! {
    pub(crate) struct EdgeId;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReactiveEdge {
    pub(crate) target: TargetNode,
}

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

#[derive(Clone)]
pub(crate) struct DependencyTransaction {
    pub(crate) observer: NodeId,
    pub(crate) previous: HashSet<TargetNode>,
    pub(crate) current: HashSet<TargetNode>,
    pub(crate) removed: HashSet<TargetNode>,
}

/// Direct indexes for the two directions of a node's reactive edges.
///
/// The edge arena remains the owner of stable edge ids, while these maps are
/// the hot-path adjacency structure. This keeps insertion, duplicate checks,
/// and removal independent of the number of neighboring edges.
pub(crate) struct NodeAdjacency {
    pub(crate) subscribers: HashMap<TargetNode, EdgeId>,
    pub(crate) dependencies: HashMap<TargetNode, EdgeId>,
}

impl NodeAdjacency {
    pub(crate) fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            dependencies: HashMap::new(),
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
pub(crate) struct EdgeIter<'a> {
    inner: Option<hash_map::Iter<'a, TargetNode, EdgeId>>,
}

impl Iterator for EdgeIter<'_> {
    type Item = (EdgeId, ReactiveEdge);

    fn next(&mut self) -> Option<Self::Item> {
        let (target, edge_id) = self.inner.as_mut()?.next()?;
        Some((*edge_id, ReactiveEdge { target: *target }))
    }
}

/// Reactive graph nodes, scheduling state, and stable storage owned by one lexical scope.
pub(crate) struct ScopeStateInner<'scope> {
    pub(crate) owner_id: OwnerId,
    pub(crate) scheduler: Rc<RefCell<GlobalScheduler>>,
    pub(crate) phase: ScopePhase,
    pub(crate) nodes: SlotMap<NodeId, NodeCore>,
    pub(crate) data: SecondaryMap<NodeId, NodeData<'scope>>,
    pub(crate) edges: SlotMap<EdgeId, ReactiveEdge>,
    pub(crate) adjacency: SecondaryMap<NodeId, NodeAdjacency>,
    pub(crate) roots: RootSet,
    pub(crate) current_owner: Option<NodeId>,
    pub(crate) root_cleanups: Vec<CleanupThunk<'scope>>,
    pub(crate) dependency_transactions: Vec<DependencyTransaction>,
    pub(crate) error_handlers: SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'scope>>,
    pub(crate) pending_error_handlers: Vec<(ErrorHandlerKey, std::ptr::NonNull<()>)>,
}

/// A reference-counted wrapper around the inner scope state.
#[derive(Clone)]
pub(crate) struct ScopeState<'scope> {
    pub(crate) inner: Rc<RefCell<ScopeStateInner<'scope>>>,
}

impl<'scope> ScopeState<'scope> {
    pub(crate) fn new(owner_id: OwnerId, scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(ScopeStateInner::new(owner_id, scheduler))),
        }
    }

    pub(crate) fn from_inner(inner: Rc<RefCell<ScopeStateInner<'scope>>>) -> Self {
        Self { inner }
    }

    pub(crate) fn into_inner(self) -> Rc<RefCell<ScopeStateInner<'scope>>> {
        self.inner
    }

    pub(crate) fn inner(&self) -> &Rc<RefCell<ScopeStateInner<'scope>>> {
        &self.inner
    }

    pub(crate) fn try_borrow(&self) -> Result<Ref<'_, ScopeStateInner<'scope>>, ReactiveError> {
        self.inner
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)
    }

    pub(crate) fn try_borrow_mut(
        &self,
    ) -> Result<RefMut<'_, ScopeStateInner<'scope>>, ReactiveError> {
        self.inner
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)
    }

    pub(crate) fn borrow(&self) -> Ref<'_, ScopeStateInner<'scope>> {
        self.inner.borrow()
    }

    pub(crate) fn borrow_mut(&self) -> RefMut<'_, ScopeStateInner<'scope>> {
        self.inner.borrow_mut()
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
    pub owner_generation: u32,
    pub active_leases: usize,
    pub queue_recovery: bool,
    pub retained_children: usize,
    pub unhandled_close_errors: usize,
    pub dropped_close_reports: usize,
}

impl<'scope> ScopeStateInner<'scope> {
    pub(crate) fn new(owner_id: OwnerId, scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        Self {
            owner_id,
            scheduler,
            phase: ScopePhase::Active,
            nodes: SlotMap::with_key(),
            data: SecondaryMap::new(),
            edges: SlotMap::with_key(),
            adjacency: SecondaryMap::new(),
            roots: RootSet::new(),
            current_owner: None,
            root_cleanups: Vec::new(),
            dependency_transactions: Vec::new(),
            error_handlers: SlotMap::with_key(),
            pending_error_handlers: Vec::new(),
        }
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn runtime_snapshot(&self) -> RuntimeSnapshot {
        let cleanups = self.root_cleanups.len()
            + self
                .data
                .values()
                .map(|data| data.cleanups.len())
                .sum::<usize>();
        let scheduler = self.scheduler.borrow();
        let active_owners = scheduler.active_owner_ids().len();
        let closing_owners = scheduler
            .active_owner_ids()
            .into_iter()
            .filter_map(|id| scheduler.get_scope_for_edge_cleanup(id))
            .filter(|state| {
                state
                    .try_borrow()
                    .is_ok_and(|state| state.phase != ScopePhase::Active)
            })
            .count();
        RuntimeSnapshot {
            nodes: self.nodes.len(),
            data: self.data.len(),
            edges: self.edges.len(),
            roots: self.roots.len(),
            cleanups,
            handlers: self
                .error_handlers
                .values()
                .filter(|entry| entry.owner.is_active())
                .count(),
            queue: scheduler.global_queue.len(),
            epoch: scheduler.current_epoch(),
            observer: active_observer_for(&self.scheduler).is_some(),
            running_queue: scheduler.running_queue,
            active_owners,
            closing_owners,
            owner_generation: self.owner_id.1,
            active_leases: scheduler.active_leases,
            queue_recovery: !scheduler.running_queue && scheduler.global_queue.is_empty(),
            retained_children: 0,
            unhandled_close_errors: scheduler.close_reports.len(),
            dropped_close_reports: scheduler.dropped_close_reports(),
        }
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
    pub(crate) fn subscriber_edges_of(&self, node_id: NodeId) -> EdgeIter<'_> {
        EdgeIter {
            inner: self
                .adjacency
                .get(node_id)
                .map(|adjacency| adjacency.subscribers.iter()),
        }
    }

    #[inline]
    pub(crate) fn dependency_edges_of(&self, node_id: NodeId) -> EdgeIter<'_> {
        EdgeIter {
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
        make_data: impl FnOnce() -> NodeData<'scope>,
    ) -> ReactiveResult<NodeId> {
        self.ensure_active()?;
        let parent = node.parent;
        let id = self.nodes.insert(node);
        self.data.insert(id, make_data());
        self.adjacency.insert(id, NodeAdjacency::new());
        self.link_child(parent, id);
        Ok(id)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.phase == ScopePhase::Active
            && self
                .scheduler
                .try_borrow()
                .is_ok_and(|scheduler| scheduler.is_scope_active(self.owner_id))
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
            && self.edges.is_empty()
            && self.adjacency.is_empty()
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

    pub(crate) fn mark_notified(&mut self, id: NodeId) -> bool {
        if !self.is_active() {
            return false;
        }
        let epoch = self.scheduler.borrow_mut().next_epoch();
        let Some(node) = self.nodes.get_mut(id) else {
            return false;
        };
        node.updated_epoch = epoch;
        node.version = node.version.wrapping_add(1);
        true
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
        value: TypedNodeRef<'scope, T>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        let epoch = self.scheduler.borrow().current_epoch();
        let mut node = NodeCore::new(NodeKindTag::Signal, parent, NodeState::Clean);
        node.updated_epoch = epoch;
        node.last_computed_epoch = epoch;
        self.register_node(node, move || {
            NodeData::new(Rc::new(NodeStorage::value(value.slot())))
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
            NodeData::new(Rc::new(NodeStorage::Computation(ComputationStorage::new(
                callback,
            ))))
        })
    }

    pub(crate) fn create_stored<T: 'scope>(
        &mut self,
        value: TypedNodeRef<'scope, T>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        self.register_node(
            NodeCore::new(NodeKindTag::Stored, parent, NodeState::Clean),
            move || NodeData::new(Rc::new(NodeStorage::value(value.slot()))),
        )
    }

    pub(crate) fn create_callback<T: 'scope, E: 'scope>(
        &mut self,
        callback: TypedNodeRef<'scope, CallbackThunk<'scope, T, E>>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        self.register_node(
            NodeCore::new(NodeKindTag::Callback, parent, NodeState::Clean),
            move || NodeData::new(Rc::new(NodeStorage::callback(callback.slot()))),
        )
    }

    pub(crate) fn create_node_ref<T: 'scope>(
        &mut self,
        value: TypedNodeRef<'scope, Option<T>>,
    ) -> ReactiveResult<NodeId> {
        let parent = self.parent_for_new_node();
        self.register_node(
            NodeCore::new(NodeKindTag::NodeRef, parent, NodeState::Clean),
            move || NodeData::new(Rc::new(NodeStorage::value(value.slot()))),
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

    pub(crate) fn remove_error_handler(
        &mut self,
        key: ErrorHandlerKey,
        identity: std::ptr::NonNull<()>,
    ) {
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

    pub(crate) fn has_value(&self, id: NodeId) -> bool {
        let Some(node) = self.nodes.get(id) else {
            return false;
        };
        match node.kind {
            NodeKindTag::Signal => true,
            NodeKindTag::Computed => self
                .data
                .get(id)
                .and_then(|data| match data.storage.as_ref() {
                    NodeStorage::Computation(storage) => storage.computation.try_peek(|behavior| {
                        behavior
                            .as_ref()
                            .is_some_and(|behavior| behavior.has_value())
                    }),
                    _ => None,
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    fn ensure_active(&self) -> Result<(), ReactiveError> {
        if self.is_active() {
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
        let mode = if self.is_active() {
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
