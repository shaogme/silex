//! Runtime node and scope-state data structures.

use super::{
    scheduler::{GlobalScheduler, ScopeId, TargetNode},
    storage::{ComputationStorage, LeaseCell, NodeStorage},
};
use crate::{
    ReactiveError, ReactiveResult,
    error::{ErrorHandlerEntry, ErrorHandlerKey},
    handle::NodeKindTag,
    internal::{
        RawId,
        value::{
            AnyValue, CallbackThunk, CleanupThunk, Computation, DerivedThunk, EffectThunk,
            MemoThunk, PreviousThunk, WatchThunk,
        },
    },
};
use slotmap::{SecondaryMap, SlotMap};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

slotmap::new_key_type! {
    pub(crate) struct EdgeId;
}

impl EdgeId {
    pub(crate) const DANGLING: Self = Self(slotmap::KeyData::from_ffi(u64::MAX));

    #[inline]
    pub(crate) fn is_dangling(self) -> bool {
        self == Self::DANGLING
    }

    #[inline]
    pub(crate) fn is_valid(self) -> bool {
        self != Self::DANGLING
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReactiveEdge {
    pub(crate) target: TargetNode,
    pub(crate) next: EdgeId,
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
    FinalCleanup,
    DisposingNodes,
    Disposed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredAccessMode {
    Active,
    FinalCleanup,
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
    pub(crate) parent: RawId,
    pub(crate) first_child: RawId,
    pub(crate) next_sibling: RawId,
    pub(crate) first_subscriber: EdgeId,
    pub(crate) first_dependency: EdgeId,
}

const _: () = assert!(std::mem::size_of::<NodeCore>() == 64);

impl NodeCore {
    pub(crate) fn new(kind: NodeKindTag, parent: Option<RawId>, state: NodeState) -> Self {
        Self {
            kind,
            state,
            running: false,
            queued: false,
            version: 0,
            updated_epoch: 0,
            last_computed_epoch: 0,
            parent: RawId::from_option(parent),
            first_child: RawId::DANGLING,
            next_sibling: RawId::DANGLING,
            first_subscriber: EdgeId::DANGLING,
            first_dependency: EdgeId::DANGLING,
        }
    }

    pub(crate) fn is_computation(&self) -> bool {
        matches!(
            self.kind,
            NodeKindTag::Effect | NodeKindTag::Memo | NodeKindTag::Derived
        )
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
    pub(crate) observer: RawId,
    pub(crate) previous: Vec<TargetNode>,
    pub(crate) current: Vec<TargetNode>,
    pub(crate) removed: Vec<TargetNode>,
}

/// Iterator over child nodes in an intra-arena sibling chain.
pub(crate) struct ChildrenIter<'a, 'scope> {
    state: &'a ScopeState<'scope>,
    curr: RawId,
}

impl Iterator for ChildrenIter<'_, '_> {
    type Item = RawId;

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
            .unwrap_or(RawId::DANGLING);
        Some(item)
    }
}

/// Iterator over edge entries in an intra-arena edge list.
pub(crate) struct EdgeIter<'a, 'scope> {
    state: &'a ScopeState<'scope>,
    curr: EdgeId,
}

impl Iterator for EdgeIter<'_, '_> {
    type Item = (EdgeId, ReactiveEdge);

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_dangling() {
            return None;
        }
        let edge_id = self.curr;
        let edge = self.state.edges.get(edge_id).copied()?;
        self.curr = edge.next;
        Some((edge_id, edge))
    }
}

/// Reactive graph nodes, scheduling state, and stable storage owned by one lexical scope.
pub(crate) struct ScopeState<'scope> {
    pub(crate) scope_id: ScopeId,
    pub(crate) scheduler: Rc<RefCell<GlobalScheduler>>,
    pub(crate) phase: ScopePhase,
    pub(crate) nodes: SlotMap<RawId, NodeCore>,
    pub(crate) data: SecondaryMap<RawId, NodeData<'scope>>,
    pub(crate) edges: SlotMap<EdgeId, ReactiveEdge>,
    pub(crate) roots: Vec<RawId>,
    pub(crate) current_owner: Option<RawId>,
    pub(crate) root_cleanups: Vec<CleanupThunk<'scope>>,
    pub(crate) dependency_transactions: Vec<DependencyTransaction>,
    pub(crate) error_handlers: SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'scope>>,
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
}

impl<'scope> ScopeState<'scope> {
    pub(crate) fn new(scope_id: ScopeId, scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        Self {
            scope_id,
            scheduler,
            phase: ScopePhase::Active,
            nodes: SlotMap::with_key(),
            data: SecondaryMap::new(),
            edges: SlotMap::with_key(),
            roots: Vec::new(),
            current_owner: None,
            root_cleanups: Vec::new(),
            dependency_transactions: Vec::new(),
            error_handlers: SlotMap::with_key(),
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
        RuntimeSnapshot {
            nodes: self.nodes.len(),
            data: self.data.len(),
            edges: self.edges.len(),
            roots: self.roots.len(),
            cleanups,
            handlers: self.error_handlers.len(),
            queue: scheduler.global_queue.len(),
            epoch: scheduler.current_epoch(),
            observer: scheduler.observer().is_some(),
            running_queue: scheduler.running_queue,
        }
    }

    pub(crate) fn parent_for_new_node(&self) -> Option<RawId> {
        self.current_owner
    }

    #[inline]
    pub(crate) fn children_of_head(&self, head: RawId) -> ChildrenIter<'_, 'scope> {
        ChildrenIter {
            state: self,
            curr: head,
        }
    }

    #[inline]
    pub(crate) fn subscriber_edges_of(&self, node_id: RawId) -> EdgeIter<'_, 'scope> {
        let first = self
            .nodes
            .get(node_id)
            .map(|n| n.first_subscriber)
            .unwrap_or(EdgeId::DANGLING);
        EdgeIter {
            state: self,
            curr: first,
        }
    }

    #[inline]
    pub(crate) fn dependency_edges_of(&self, node_id: RawId) -> EdgeIter<'_, 'scope> {
        let first = self
            .nodes
            .get(node_id)
            .map(|n| n.first_dependency)
            .unwrap_or(EdgeId::DANGLING);
        EdgeIter {
            state: self,
            curr: first,
        }
    }

    pub(crate) fn link_child(&mut self, parent: RawId, child: RawId) {
        if parent.is_dangling() {
            self.roots.push(child);
            return;
        }
        let old_first = self
            .nodes
            .get(parent)
            .map(|p| p.first_child)
            .unwrap_or(RawId::DANGLING);
        if let Some(child_node) = self.nodes.get_mut(child) {
            child_node.next_sibling = old_first;
        }
        if let Some(parent_node) = self.nodes.get_mut(parent) {
            parent_node.first_child = child;
        }
    }

    pub(crate) fn unlink_child(&mut self, parent: RawId, child: RawId, child_next_sibling: RawId) {
        if parent.is_dangling() {
            self.roots.retain(|&root| root != child);
            return;
        }
        let Some(parent_node) = self.nodes.get_mut(parent) else {
            return;
        };
        if parent_node.first_child == child {
            parent_node.first_child = child_next_sibling;
            return;
        }
        let mut curr = parent_node.first_child;
        while curr.is_valid() {
            let next = self
                .nodes
                .get(curr)
                .map(|n| n.next_sibling)
                .unwrap_or(RawId::DANGLING);
            if next == child {
                if let Some(curr_node) = self.nodes.get_mut(curr) {
                    curr_node.next_sibling = child_next_sibling;
                }
                break;
            }
            curr = next;
        }
    }

    pub(crate) fn register(
        &mut self,
        node: NodeCore,
        make_data: impl FnOnce() -> NodeData<'scope>,
    ) -> ReactiveResult<RawId> {
        self.ensure_active()?;
        let parent = node.parent;
        let id = self.nodes.insert(node);
        self.data.insert(id, make_data());
        self.link_child(parent, id);
        Ok(id)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.phase == ScopePhase::Active && self.scheduler.borrow().is_scope_active(self.scope_id)
    }

    pub(crate) fn try_is_active(&self) -> ReactiveResult<bool> {
        if self.phase != ScopePhase::Active {
            return Ok(false);
        }
        Ok(self
            .scheduler
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .is_scope_active(self.scope_id))
    }

    pub(crate) fn begin_final_cleanup(&mut self) -> bool {
        if self.phase != ScopePhase::Active {
            return false;
        }
        self.phase = ScopePhase::FinalCleanup;
        true
    }

    pub(crate) fn begin_node_disposal(&mut self) {
        assert_eq!(
            self.phase,
            ScopePhase::FinalCleanup,
            "node disposal must begin after final cleanup",
        );
        self.phase = ScopePhase::DisposingNodes;
    }

    pub(crate) fn finish_dispose(&mut self) {
        self.current_owner = None;
        self.phase = ScopePhase::Disposed;
    }

    pub(crate) fn allows_final_cleanup_stored_access(&self) -> bool {
        self.phase == ScopePhase::FinalCleanup
    }

    pub(crate) fn node_exists(&self, id: RawId) -> bool {
        self.nodes.get(id).is_some()
    }

    pub(crate) fn mark_notified(&mut self, id: RawId) -> bool {
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

    pub(crate) fn set_context(&mut self, owner: Option<RawId>) {
        self.current_owner = owner;
    }

    pub(crate) fn create_signal(&mut self, value: AnyValue<'scope>) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        let epoch = self.scheduler.borrow().current_epoch();
        let mut node = NodeCore::new(NodeKindTag::Signal, parent, NodeState::Clean);
        node.updated_epoch = epoch;
        node.last_computed_epoch = epoch;
        self.register(node, move || {
            NodeData::new(Rc::new(NodeStorage::Value(LeaseCell::new(value))))
        })
    }

    pub(super) fn register_effect(
        &mut self,
        callback: EffectThunk<'scope>,
    ) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        self.register(
            NodeCore::new(NodeKindTag::Effect, parent, NodeState::Dirty),
            move || {
                NodeData::new(Rc::new(NodeStorage::Computation(ComputationStorage::new(
                    Computation::Effect(callback),
                ))))
            },
        )
    }

    pub(super) fn register_previous(
        &mut self,
        callback: PreviousThunk<'scope>,
    ) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        self.register(
            NodeCore::new(NodeKindTag::Effect, parent, NodeState::Dirty),
            move || {
                NodeData::new(Rc::new(NodeStorage::Computation(ComputationStorage::new(
                    Computation::Previous(callback),
                ))))
            },
        )
    }

    pub(super) fn register_watch(&mut self, callback: WatchThunk<'scope>) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        self.register(
            NodeCore::new(NodeKindTag::Effect, parent, NodeState::Dirty),
            move || {
                NodeData::new(Rc::new(NodeStorage::Computation(ComputationStorage::new(
                    Computation::Watch(callback),
                ))))
            },
        )
    }

    pub(super) fn register_memo(
        &mut self,
        callback: MemoThunk<'scope>,
        derived: bool,
    ) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        let kind = if derived {
            NodeKindTag::Derived
        } else {
            NodeKindTag::Memo
        };
        self.register(NodeCore::new(kind, parent, NodeState::Dirty), move || {
            NodeData::new(Rc::new(NodeStorage::Computation(ComputationStorage::new(
                Computation::Memo(callback),
            ))))
        })
    }

    pub(super) fn register_derived(
        &mut self,
        callback: DerivedThunk<'scope>,
    ) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        self.register(
            NodeCore::new(NodeKindTag::Derived, parent, NodeState::Dirty),
            move || {
                NodeData::new(Rc::new(NodeStorage::Computation(ComputationStorage::new(
                    Computation::Derived(callback),
                ))))
            },
        )
    }

    pub(crate) fn create_stored(&mut self, value: AnyValue<'scope>) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        self.register(
            NodeCore::new(NodeKindTag::Stored, parent, NodeState::Clean),
            move || NodeData::new(Rc::new(NodeStorage::Value(LeaseCell::new(value)))),
        )
    }

    pub(crate) fn create_callback(
        &mut self,
        callback: CallbackThunk<'scope>,
    ) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        self.register(
            NodeCore::new(NodeKindTag::Callback, parent, NodeState::Clean),
            move || NodeData::new(Rc::new(NodeStorage::Callback(LeaseCell::new(callback)))),
        )
    }

    pub(crate) fn create_node_ref(&mut self, value: AnyValue<'scope>) -> ReactiveResult<RawId> {
        let parent = self.parent_for_new_node();
        self.register(
            NodeCore::new(NodeKindTag::NodeRef, parent, NodeState::Clean),
            move || NodeData::new(Rc::new(NodeStorage::Value(LeaseCell::new(value)))),
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
        if !self.try_is_active()? {
            return Err(ReactiveError::NoSuchNode);
        }
        Ok(self.error_handlers.insert(entry))
    }

    pub(crate) fn take_error_handlers(
        &mut self,
    ) -> SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'scope>> {
        std::mem::take(&mut self.error_handlers)
    }

    pub(crate) fn drop_error_handlers(
        handlers: SlotMap<ErrorHandlerKey, ErrorHandlerEntry<'scope>>,
    ) {
        let mut first_panic = None;
        for (_, entry) in handlers {
            if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(entry)))
                && first_panic.is_none()
            {
                first_panic = Some(panic);
            }
        }
        if let Some(panic) = first_panic {
            resume_unwind(panic);
        }
    }

    pub(crate) fn has_value(&self, id: RawId) -> bool {
        let Some(node) = self.nodes.get(id) else {
            return false;
        };
        match node.kind {
            NodeKindTag::Signal => true,
            NodeKindTag::Memo | NodeKindTag::Derived => self
                .data
                .get(id)
                .and_then(|data| match data.storage.as_ref() {
                    NodeStorage::Computation(storage) => Some(storage.value.is_initialized()),
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
        id: RawId,
        reactive: bool,
    ) -> ReactiveResult<Rc<NodeStorage<'scope>>> {
        self.ensure_active()?;
        let node = self.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        let valid_kind = if reactive {
            matches!(
                node.kind,
                NodeKindTag::Signal | NodeKindTag::Memo | NodeKindTag::Derived
            )
        } else {
            node.kind == NodeKindTag::Signal
        };
        if !valid_kind {
            return Err(ReactiveError::WrongKind);
        }
        let data = self.data.get(id).ok_or(ReactiveError::NoSuchNode)?;
        let valid_storage = match node.kind {
            NodeKindTag::Signal => matches!(data.storage.as_ref(), NodeStorage::Value(_)),
            NodeKindTag::Memo | NodeKindTag::Derived => {
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
        id: RawId,
    ) -> ReactiveResult<(Rc<NodeStorage<'scope>>, StoredAccessMode)> {
        let mode = if self.is_active() {
            StoredAccessMode::Active
        } else if self.allows_final_cleanup_stored_access() {
            StoredAccessMode::FinalCleanup
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

    pub(crate) fn node_ref_storage(&self, id: RawId) -> ReactiveResult<Rc<NodeStorage<'scope>>> {
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

    pub(crate) fn callback_storage(&self, id: RawId) -> ReactiveResult<Rc<NodeStorage<'scope>>> {
        self.ensure_active()?;
        let node = self.nodes.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if node.kind != NodeKindTag::Callback {
            return Err(ReactiveError::WrongKind);
        }
        let data = self.data.get(id).ok_or(ReactiveError::NoSuchNode)?;
        if !matches!(data.storage.as_ref(), NodeStorage::Callback(_)) {
            return Err(ReactiveError::WrongKind);
        }
        Ok(data.storage.clone())
    }
}
