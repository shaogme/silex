//! Global scheduler, execution contexts, and scope lifetime tracking.

use super::model::ScopeState;
use crate::{
    ReactiveError, ReactiveResult,
    borrow::{BorrowCell, BorrowSite, SharedCell},
    error::ErrorContext,
    internal::NodeId,
    root::CloseError,
    unsafe_boundary::{ActiveOwnerProof, CleanupOwnerProof, ErasedErrorEvent, WeakOwnerToken},
};

use std::{
    cell::Cell,
    collections::{HashSet, VecDeque},
    mem,
    rc::Rc,
};

/// Close diagnostics that cannot be returned through the current call stack.
///
/// The queue is intentionally runtime-owned and does not invoke user code.
/// `dropped` is retained as an invariant diagnostic for the narrow case where
/// an internal borrow conflict prevents recording the error itself.
pub(crate) struct CloseReportQueue {
    errors: BorrowCell<VecDeque<CloseError>>,
    dropped: Cell<usize>,
}

impl CloseReportQueue {
    pub(crate) fn new() -> Rc<Self> {
        Rc::new(Self {
            errors: BorrowCell::new(VecDeque::new(), BorrowSite::CloseReport),
            dropped: Cell::new(0),
        })
    }

    pub(crate) fn push(&self, error: CloseError) {
        match self.errors.try_write() {
            Ok(mut errors) => errors.push_back(error),
            Err(_) => {
                self.dropped.set(self.dropped.get().saturating_add(1));
            }
        }
    }

    pub(crate) fn take(&self) -> ReactiveResult<Vec<CloseError>> {
        Ok(self
            .errors
            .try_write()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .drain(..)
            .collect())
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn len(&self) -> ReactiveResult<usize> {
        Ok(self
            .errors
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .len())
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn dropped(&self) -> usize {
        self.dropped.get()
    }
}

/// Generational runtime identity for an owner slot.
///
/// The slot is reused only after the owner has reached its release invariant;
/// releasing it increments the generation so stale handles cannot address the
/// replacement owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct OwnerId(pub(crate) u32, pub(crate) u64);

impl OwnerId {
    pub(crate) const fn initial(slot: u32) -> Self {
        Self(slot, 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TargetNode {
    pub(crate) owner_id: OwnerId,
    pub(crate) node: NodeId,
}

pub(crate) struct DeferredErrorEvent {
    pub(crate) source_owner: OwnerId,
    pub(crate) sequence: u64,
    pub(crate) context: ErrorContext,
    pub(crate) event: ErasedErrorEvent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Observer {
    pub(crate) owner_id: OwnerId,
    pub(crate) node: NodeId,
}

#[derive(Clone)]
pub(crate) struct ExecutionContext {
    pub(crate) scheduler: SharedCell<GlobalScheduler>,
    pub(crate) observer: Option<Observer>,
    pub(crate) blocked_scopes: Vec<OwnerId>,
    pub(crate) runtime_boundary: bool,
}

thread_local! {
    static ACTIVE_CONTEXT: BorrowCell<Vec<ExecutionContext>> =
        const { BorrowCell::new(Vec::new(), BorrowSite::ObserverStack) };
    static OBSERVER_RECOVERY_FAILURES: Cell<usize> = const { Cell::new(0) };
}

pub(crate) fn validate_active_scheduler(
    scheduler: &SharedCell<GlobalScheduler>,
) -> Result<(), ReactiveError> {
    ACTIVE_CONTEXT.with(|stack| {
        let contexts = stack
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        if contexts
            .iter()
            .rev()
            .any(|context| context.runtime_boundary && !Rc::ptr_eq(&context.scheduler, scheduler))
        {
            return Err(ReactiveError::RuntimeMismatch);
        }
        for context in contexts.iter().rev() {
            if Rc::ptr_eq(&context.scheduler, scheduler) {
                return Ok(());
            }
            if context.runtime_boundary || context.observer.is_some() {
                return Err(ReactiveError::RuntimeMismatch);
            }
        }
        Ok(())
    })
}

pub(crate) fn has_runtime_boundary() -> ReactiveResult<bool> {
    ACTIVE_CONTEXT.with(|stack| {
        Ok(stack
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .iter()
            .any(|context| context.runtime_boundary))
    })
}

/// Return the context relevant to `scheduler` from the current execution stack.
///
/// A frame for another scheduler only contributes a foreign observer. An
/// untracked frame is intentionally skipped, so untracking one runtime does
/// not disable tracking for every runtime on this thread.
pub(crate) fn active_ctx(
    scheduler: &SharedCell<GlobalScheduler>,
) -> ReactiveResult<Option<ExecutionContext>> {
    ACTIVE_CONTEXT.with(|stack| {
        Ok(stack
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .iter()
            .rev()
            .find_map(|context| {
                if Rc::ptr_eq(&context.scheduler, scheduler) || context.observer.is_some() {
                    Some(context.clone())
                } else {
                    None
                }
            }))
    })
}

pub(crate) fn active_observer_contexts(
    scheduler: &SharedCell<GlobalScheduler>,
) -> ReactiveResult<Vec<ExecutionContext>> {
    ACTIVE_CONTEXT.with(|stack| {
        Ok(stack
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .iter()
            .filter(|context| {
                Rc::ptr_eq(&context.scheduler, scheduler) && context.observer.is_some()
            })
            .cloned()
            .collect())
    })
}

#[cfg(feature = "test-support")]
pub(crate) fn active_observer_for(
    scheduler: &SharedCell<GlobalScheduler>,
) -> ReactiveResult<Option<Observer>> {
    active_ctx(scheduler).map(|ctx| {
        ctx.and_then(|ctx| {
            ctx.observer
                .filter(|_| Rc::ptr_eq(&ctx.scheduler, scheduler))
        })
    })
}

#[cfg(feature = "test-support")]
pub(crate) fn observer_recovery_failures() -> usize {
    OBSERVER_RECOVERY_FAILURES.with(Cell::get)
}

/// Restores the previous execution context when the surrounding operation
/// unwinds. The stack is thread-local, but every frame is owned by one
/// scheduler and runtime state remains owned by that scheduler.
pub(crate) struct ObserverFrame {
    active: bool,
}

impl ObserverFrame {
    pub(crate) fn push(
        scheduler: SharedCell<GlobalScheduler>,
        observer: Option<Observer>,
    ) -> ReactiveResult<Self> {
        ACTIVE_CONTEXT.with(|stack| {
            stack
                .try_write()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .push(ExecutionContext {
                    scheduler,
                    observer,
                    blocked_scopes: Vec::new(),
                    runtime_boundary: false,
                });
            Ok(Self { active: true })
        })
    }

    pub(crate) fn push_untracked(scheduler: SharedCell<GlobalScheduler>) -> ReactiveResult<Self> {
        ACTIVE_CONTEXT.with(|stack| {
            stack
                .try_write()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .push(ExecutionContext {
                    scheduler,
                    observer: None,
                    blocked_scopes: Vec::new(),
                    runtime_boundary: false,
                });
            Ok(Self { active: true })
        })
    }

    pub(crate) fn push_child(
        scheduler: SharedCell<GlobalScheduler>,
        owner_id: OwnerId,
    ) -> ReactiveResult<Self> {
        let inherited = active_ctx(&scheduler)?.and_then(|mut ctx| {
            if !Rc::ptr_eq(&ctx.scheduler, &scheduler) {
                return None;
            }
            let runtime_boundary = ctx.runtime_boundary;
            let observer = ctx.observer.take();
            if let Some(observer) = observer {
                ctx.blocked_scopes.push(owner_id);
                Some(ExecutionContext {
                    scheduler: scheduler.clone(),
                    observer: Some(observer),
                    blocked_scopes: ctx.blocked_scopes,
                    runtime_boundary,
                })
            } else if runtime_boundary {
                Some(ExecutionContext {
                    scheduler: scheduler.clone(),
                    observer: None,
                    blocked_scopes: Vec::new(),
                    runtime_boundary: true,
                })
            } else {
                None
            }
        });
        ACTIVE_CONTEXT.with(|stack| {
            stack
                .try_write()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .push(inherited.unwrap_or(ExecutionContext {
                    scheduler,
                    observer: None,
                    blocked_scopes: Vec::new(),
                    runtime_boundary: false,
                }));
            Ok(Self { active: true })
        })
    }

    pub(crate) fn push_runtime_boundary(
        scheduler: SharedCell<GlobalScheduler>,
    ) -> ReactiveResult<Self> {
        ACTIVE_CONTEXT.with(|stack| {
            stack
                .try_write()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .push(ExecutionContext {
                    scheduler,
                    observer: None,
                    blocked_scopes: Vec::new(),
                    runtime_boundary: true,
                });
            Ok(Self { active: true })
        })
    }
}

impl Drop for ObserverFrame {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        ACTIVE_CONTEXT.with(|stack| {
            let Ok(mut stack) = stack.try_write() else {
                OBSERVER_RECOVERY_FAILURES.with(|count| {
                    count.set(count.get().saturating_add(1));
                });
                return;
            };
            if stack.pop().is_none() {
                OBSERVER_RECOVERY_FAILURES.with(|count| {
                    count.set(count.get().saturating_add(1));
                });
            }
        });
    }
}

/// Prevents queue flushing while a computation is still provisional.
///
/// Initial callbacks may register children and cleanups that can write to
/// other signals while the provisional node is being rolled back. A nested
/// guard keeps that invariant intact for computations created by the callback.
pub(crate) struct InitialFlushGuard {
    scheduler: SharedCell<GlobalScheduler>,
}

impl InitialFlushGuard {
    pub(crate) fn try_new(scheduler: SharedCell<GlobalScheduler>) -> Result<Self, ReactiveError> {
        let mut scheduler_ref = scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        scheduler_ref.initial_flush_depth = scheduler_ref.initial_flush_depth.saturating_add(1);
        drop(scheduler_ref);
        Ok(Self { scheduler })
    }
}

impl Drop for InitialFlushGuard {
    fn drop(&mut self) {
        if let Ok(mut scheduler) = self.scheduler.try_borrow_mut() {
            scheduler.initial_flush_depth = scheduler.initial_flush_depth.saturating_sub(1);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScheduledTask {
    pub(crate) owner_id: OwnerId,
    pub(crate) node: NodeId,
}

pub(crate) struct ScopeEntry {
    owner: WeakOwnerToken,
    parent: Option<OwnerId>,
    mode: OwnerMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnerMode {
    Root,
    Persistent,
    Transient,
}

enum OwnerSlotState {
    Vacant,
    Active(ScopeEntry),
    Inactive(ScopeEntry),
    Retired,
}

struct OwnerSlot {
    generation: u64,
    state: OwnerSlotState,
}

struct OwnerSlotTable {
    slots: Vec<OwnerSlot>,
    free: Vec<u32>,
    next_slot: u32,
}

impl OwnerSlotTable {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            next_slot: 0,
        }
    }

    fn alloc<'scope>(
        &mut self,
        state: &ScopeState<'scope>,
        parent: Option<OwnerId>,
        mode: OwnerMode,
    ) -> ReactiveResult<OwnerId> {
        let slot_id = if let Some(slot_id) = self.free.pop() {
            let slot = self
                .slots
                .get(slot_id as usize)
                .ok_or(ReactiveError::InvariantViolation)?;
            if !matches!(slot.state, OwnerSlotState::Vacant) {
                return Err(ReactiveError::InvariantViolation);
            }
            slot_id
        } else {
            let slot_id = self.next_slot;
            self.next_slot = self
                .next_slot
                .checked_add(1)
                .ok_or(ReactiveError::InvariantViolation)?;
            self.slots.push(OwnerSlot {
                generation: 0,
                state: OwnerSlotState::Vacant,
            });
            slot_id
        };

        let slot = self
            .slots
            .get_mut(slot_id as usize)
            .ok_or(ReactiveError::InvariantViolation)?;
        let generation = slot.generation;
        if !matches!(slot.state, OwnerSlotState::Vacant) {
            return Err(ReactiveError::InvariantViolation);
        }
        slot.state = OwnerSlotState::Active(ScopeEntry {
            owner: WeakOwnerToken::from_typed(state),
            parent,
            mode,
        });
        Ok(OwnerId(slot_id, generation))
    }

    fn deactivate(&mut self, id: OwnerId) -> bool {
        let Some(slot) = self.slots.get_mut(id.0 as usize) else {
            return false;
        };
        if slot.generation != id.1 {
            return false;
        }
        let state = mem::replace(&mut slot.state, OwnerSlotState::Retired);
        match state {
            OwnerSlotState::Active(entry) => {
                slot.state = OwnerSlotState::Inactive(entry);
                true
            }
            other => {
                slot.state = other;
                false
            }
        }
    }

    fn release(&mut self, id: OwnerId) {
        let Some(slot) = self.slots.get_mut(id.0 as usize) else {
            return;
        };
        if slot.generation != id.1 {
            return;
        }
        let state = mem::replace(&mut slot.state, OwnerSlotState::Retired);
        if !matches!(state, OwnerSlotState::Inactive(_)) {
            slot.state = state;
            return;
        }
        if id.1 == u64::MAX {
            return;
        }
        slot.generation = id.1.saturating_add(1);
        slot.state = OwnerSlotState::Vacant;
        self.free.push(id.0);
    }

    fn is_active(&self, id: OwnerId) -> bool {
        self.slots.get(id.0 as usize).is_some_and(|slot| {
            slot.generation == id.1 && matches!(slot.state, OwnerSlotState::Active(_))
        })
    }

    fn entry(&self, id: OwnerId) -> Option<&ScopeEntry> {
        let slot = self.slots.get(id.0 as usize)?;
        if slot.generation != id.1 {
            return None;
        }
        match &slot.state {
            OwnerSlotState::Active(entry) | OwnerSlotState::Inactive(entry) => Some(entry),
            OwnerSlotState::Vacant | OwnerSlotState::Retired => None,
        }
    }

    #[cfg(feature = "test-support")]
    fn active_ids(&self) -> Vec<OwnerId> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                if matches!(slot.state, OwnerSlotState::Active(_)) {
                    Some(OwnerId(index as u32, slot.generation))
                } else {
                    None
                }
            })
            .collect()
    }

    #[cfg(test)]
    fn free_len(&self) -> usize {
        self.free.len()
    }
}

pub(crate) struct GlobalScheduler {
    owner_slots: OwnerSlotTable,
    epoch: u64,
    pub(crate) global_queue: VecDeque<ScheduledTask>,
    pub(crate) worklist: VecDeque<ScheduledTask>,
    pub(crate) running_queue: bool,
    pub(crate) batch_depth: usize,
    pub(crate) evaluating: usize,
    pub(crate) executing: usize,
    pub(crate) active_leases: usize,
    disposal_depth: usize,
    pending_endpoints: VecDeque<TargetNode>,
    pending_endpoint_ids: HashSet<TargetNode>,
    initial_flush_depth: usize,
    deferred_errors: VecDeque<DeferredErrorEvent>,
    next_error_sequence: u64,
    pub(crate) close_reports: Rc<CloseReportQueue>,
}

impl GlobalScheduler {
    #[cfg(test)]
    pub(crate) fn new() -> SharedCell<Self> {
        Self::new_with_reporter(CloseReportQueue::new())
    }

    pub(crate) fn new_with_reporter(reporter: Rc<CloseReportQueue>) -> SharedCell<Self> {
        Rc::new(BorrowCell::new(
            Self {
                owner_slots: OwnerSlotTable::new(),
                epoch: 1,
                global_queue: VecDeque::new(),
                worklist: VecDeque::new(),
                running_queue: false,
                batch_depth: 0,
                evaluating: 0,
                executing: 0,
                active_leases: 0,
                disposal_depth: 0,
                pending_endpoints: VecDeque::new(),
                pending_endpoint_ids: HashSet::new(),
                initial_flush_depth: 0,
                deferred_errors: VecDeque::new(),
                next_error_sequence: 0,
                close_reports: reporter,
            },
            BorrowSite::Scheduler,
        ))
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn dropped_close_reports(&self) -> usize {
        self.close_reports.dropped()
    }

    pub(crate) fn current_epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) fn next_epoch(&mut self) -> u64 {
        self.epoch = self.epoch.wrapping_add(1).max(1);
        self.epoch
    }

    pub(crate) fn alloc_owner<'scope>(
        &mut self,
        state: &ScopeState<'scope>,
        parent: Option<OwnerId>,
        mode: OwnerMode,
    ) -> ReactiveResult<OwnerId> {
        self.owner_slots.alloc(state, parent, mode)
    }

    pub(crate) fn deactivate_scope(&mut self, id: OwnerId) {
        if !self.owner_slots.deactivate(id) {
            return;
        }
        self.global_queue.retain(|task| task.owner_id != id);
        self.worklist.retain(|task| task.owner_id != id);
        self.deferred_errors
            .retain(|event| event.source_owner != id);
    }

    pub(crate) fn release_owner_id(&mut self, id: OwnerId) {
        self.owner_slots.release(id);
    }

    pub(crate) fn is_scope_active(&self, id: OwnerId) -> bool {
        self.owner_slots.is_active(id)
    }

    pub(crate) fn is_scope_current(&self, id: OwnerId, expected: &WeakOwnerToken) -> bool {
        if !self.is_scope_active(id) {
            return false;
        }
        self.owner_slots
            .entry(id)
            .is_some_and(|entry| entry.owner.ptr_eq(expected) && self.is_scope_active(id))
    }

    /// Resolve an active owner proof after validating the complete registry
    /// identity and active phase.
    pub(crate) fn resolve_active_owner<'scope>(
        &self,
        id: OwnerId,
        expected: &WeakOwnerToken,
    ) -> ReactiveResult<Option<ActiveOwnerProof<'scope>>> {
        let Some(entry) = self.owner_slots.entry(id) else {
            return Ok(None);
        };
        if !self.is_scope_active(id) || !entry.owner.ptr_eq(expected) {
            return Ok(None);
        }
        let Some(state) = entry.owner.upgrade_erased() else {
            return Ok(None);
        };
        ActiveOwnerProof::from_registry(id, id.1, expected, state)
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn active_owner_ids(&self) -> Vec<OwnerId> {
        self.owner_slots.active_ids()
    }

    pub(crate) fn get_scope<'scope>(
        &self,
        id: OwnerId,
    ) -> ReactiveResult<Option<ScopeState<'scope>>> {
        let Some(entry) = self.owner_slots.entry(id) else {
            return Ok(None);
        };
        if !self.is_scope_active(id) {
            return Ok(None);
        }
        let Some(state) = entry.owner.upgrade_erased() else {
            return Ok(None);
        };
        ActiveOwnerProof::from_registry(id, id.1, &entry.owner, state)
            .map(|proof| proof.map(|proof| proof.state()))
    }

    pub(crate) fn get_scope_for_edge_cleanup<'scope>(
        &self,
        id: OwnerId,
    ) -> ReactiveResult<Option<ScopeState<'scope>>> {
        self.resolve_cleanup_owner(id)
            .map(|proof| proof.map(|proof| proof.state()))
    }

    pub(crate) fn resolve_cleanup_owner<'scope>(
        &self,
        id: OwnerId,
    ) -> ReactiveResult<Option<CleanupOwnerProof<'scope>>> {
        let Some(entry) = self.owner_slots.entry(id) else {
            return Ok(None);
        };
        let Some(state) = entry.owner.upgrade_erased() else {
            return Ok(None);
        };
        CleanupOwnerProof::from_registry(id, id.1, &entry.owner, state)
    }

    pub(crate) fn owner_metadata(&self, id: OwnerId) -> Option<(OwnerMode, Option<OwnerId>)> {
        self.owner_slots
            .entry(id)
            .map(|entry| (entry.mode, entry.parent))
    }

    pub(crate) fn enqueue_effect(&mut self, task: ScheduledTask) {
        if self.is_scope_active(task.owner_id) {
            self.global_queue.push_back(task);
        }
    }

    pub(crate) fn enqueue_deferred_error(
        &mut self,
        source_owner: OwnerId,
        context: ErrorContext,
        event: ErasedErrorEvent,
    ) {
        if !self.is_scope_active(source_owner) {
            return;
        }
        let sequence = self.next_error_sequence;
        self.next_error_sequence = self.next_error_sequence.saturating_add(1);
        self.deferred_errors.push_back(DeferredErrorEvent {
            source_owner,
            sequence,
            context,
            event,
        });
    }

    pub(crate) fn take_deferred_errors(&mut self) -> Vec<DeferredErrorEvent> {
        let mut events: Vec<_> = self.deferred_errors.drain(..).collect();
        events.sort_by_key(|event| event.sequence);
        events
    }

    pub(crate) fn has_deferred_errors(&self) -> bool {
        !self.deferred_errors.is_empty()
    }

    pub(crate) fn cancel_effect(&mut self, target: TargetNode) {
        self.global_queue
            .retain(|task| task.owner_id != target.owner_id || task.node != target.node);
        self.worklist
            .retain(|task| task.owner_id != target.owner_id || task.node != target.node);
    }

    pub(crate) fn begin_disposal(&mut self) -> bool {
        let outermost = self.disposal_depth == 0;
        self.disposal_depth = self.disposal_depth.saturating_add(1);
        outermost
    }

    pub(crate) fn end_disposal(&mut self) {
        debug_assert!(self.disposal_depth > 0);
        self.disposal_depth = self.disposal_depth.saturating_sub(1);
    }

    pub(crate) fn is_disposing(&self) -> bool {
        self.disposal_depth > 0
    }

    pub(crate) fn enqueue_pending_endpoint(&mut self, target: TargetNode) {
        if self.pending_endpoint_ids.insert(target) {
            self.pending_endpoints.push_back(target);
        }
    }

    pub(crate) fn take_pending_endpoints(&mut self) -> Vec<TargetNode> {
        let pending = self.pending_endpoints.drain(..).collect();
        self.pending_endpoint_ids.clear();
        pending
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.batch_depth == 0 && self.evaluating == 0 && self.executing == 0 && !self.running_queue
    }

    pub(crate) fn should_flush(&self) -> bool {
        self.initial_flush_depth == 0
            && self.is_idle()
            && self.active_leases == 0
            && (!self.global_queue.is_empty()
                || !self.worklist.is_empty()
                || !self.deferred_errors.is_empty())
    }
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
    use crate::error::ErrorEvent;
    use crate::runtime::ScopeState;

    #[test]
    fn untracked_frame_only_masks_its_scheduler() {
        let tracked_scheduler = GlobalScheduler::new();
        let untracked_scheduler = GlobalScheduler::new();
        let observer = ObserverFrame::push(
            tracked_scheduler.clone(),
            Some(Observer {
                owner_id: OwnerId::initial(0),
                node: NodeId::DANGLING,
            }),
        )
        .expect("observer frame setup");
        let untracked = ObserverFrame::push_untracked(untracked_scheduler.clone())
            .expect("untracked frame setup");

        assert!(
            active_ctx(&tracked_scheduler)
                .expect("active context read")
                .is_some_and(|context| context.observer.is_some())
        );
        assert!(
            active_ctx(&untracked_scheduler)
                .expect("active context read")
                .is_some_and(|context| context.observer.is_none())
        );

        drop(untracked);
        drop(observer);
    }

    #[test]
    fn pending_endpoint_queue_deduplicates_until_the_outer_disposal_drains_it() {
        let scheduler = GlobalScheduler::new();
        let target = TargetNode {
            owner_id: OwnerId::initial(3),
            node: NodeId::DANGLING,
        };
        let other = TargetNode {
            owner_id: OwnerId::initial(4),
            node: NodeId::DANGLING,
        };

        let outermost = scheduler
            .try_borrow_mut()
            .expect("scheduler borrow")
            .begin_disposal();
        assert!(outermost);
        {
            let mut scheduler = scheduler.try_borrow_mut().expect("scheduler borrow");
            assert!(scheduler.is_disposing());
            scheduler.enqueue_pending_endpoint(target);
            scheduler.enqueue_pending_endpoint(target);
            scheduler.enqueue_pending_endpoint(other);
        }
        let pending = scheduler
            .try_borrow_mut()
            .expect("scheduler borrow")
            .take_pending_endpoints();
        assert_eq!(pending, vec![target, other]);
        scheduler
            .try_borrow_mut()
            .expect("scheduler borrow")
            .end_disposal();
        assert!(
            !scheduler
                .try_borrow()
                .expect("scheduler borrow")
                .is_disposing()
        );
    }

    #[test]
    fn deferred_error_events_are_sequenced_and_dropped_on_owner_deactivation() {
        let scheduler = GlobalScheduler::new();
        let state = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let owner_id = scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .alloc_owner(&state, None, OwnerMode::Transient)
            .expect("owner allocation");
        state.try_borrow_mut().expect("state write").owner_id = owner_id;

        {
            let mut scheduler_ref = scheduler.try_borrow_mut().expect("scheduler write");
            scheduler_ref.enqueue_deferred_error(
                owner_id,
                ErrorContext::new("first"),
                ErasedErrorEvent::from_typed(ErrorEvent::invariant("first")),
            );
            scheduler_ref.enqueue_deferred_error(
                owner_id,
                ErrorContext::new("second"),
                ErasedErrorEvent::from_typed(ErrorEvent::invariant("second")),
            );
        }

        let events = scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .take_deferred_errors();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[1].sequence, 1);

        let mut scheduler_ref = scheduler.try_borrow_mut().expect("scheduler write");
        scheduler_ref.enqueue_deferred_error(
            owner_id,
            ErrorContext::new("dropped"),
            ErasedErrorEvent::from_typed(ErrorEvent::invariant("dropped")),
        );
        scheduler_ref.deactivate_scope(owner_id);
        assert!(scheduler_ref.take_deferred_errors().is_empty());
    }

    #[test]
    fn foreign_observer_remains_visible_for_runtime_validation() {
        let observer_scheduler = GlobalScheduler::new();
        let source_scheduler = GlobalScheduler::new();
        let observer_frame = ObserverFrame::push(
            observer_scheduler.clone(),
            Some(Observer {
                owner_id: OwnerId::initial(0),
                node: NodeId::DANGLING,
            }),
        )
        .expect("observer frame setup");

        let context = active_ctx(&source_scheduler)
            .expect("active context read")
            .expect("foreign observer context");
        assert_eq!(
            context.observer.expect("observer").owner_id,
            OwnerId::initial(0)
        );
        assert!(Rc::ptr_eq(&context.scheduler, &observer_scheduler));

        drop(observer_frame);
    }

    #[test]
    fn scope_slots_are_reused_only_after_release() {
        let scheduler = GlobalScheduler::new();
        let first = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let first_id = scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .alloc_owner(&first, None, OwnerMode::Transient)
            .expect("owner allocation");
        first.try_borrow_mut().expect("state write").owner_id = first_id;
        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .deactivate_scope(first_id);
        assert!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .get_scope_for_edge_cleanup(first_id)
                .expect("cleanup proof lookup")
                .is_some()
        );

        let second = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let second_id = scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .alloc_owner(&second, None, OwnerMode::Transient)
            .expect("owner allocation");
        assert_ne!(first_id, second_id);

        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .release_owner_id(first_id);
        assert!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .get_scope_for_edge_cleanup(first_id)
                .expect("released owner lookup")
                .is_none()
        );

        let third = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let third_id = scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .alloc_owner(&third, None, OwnerMode::Transient)
            .expect("owner allocation");

        assert_eq!(first_id.0, third_id.0);
        assert_ne!(first_id.1, third_id.1);

        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .deactivate_scope(second_id);
        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .release_owner_id(second_id);
        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .deactivate_scope(third_id);
        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .release_owner_id(third_id);
    }

    #[test]
    fn release_is_idempotent_and_rejects_stale_generation() {
        let scheduler = GlobalScheduler::new();
        let first = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let first_id = scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .alloc_owner(&first, None, OwnerMode::Transient)
            .expect("owner allocation");
        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .deactivate_scope(first_id);
        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .release_owner_id(first_id);
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .owner_slots
                .free_len(),
            1
        );

        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .release_owner_id(first_id);
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .owner_slots
                .free_len(),
            1
        );

        let replacement = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let replacement_id = scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .alloc_owner(&replacement, None, OwnerMode::Transient)
            .expect("owner allocation");
        scheduler
            .try_borrow_mut()
            .expect("scheduler write")
            .release_owner_id(first_id);
        assert!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .is_scope_active(replacement_id)
        );
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .owner_slots
                .free_len(),
            0
        );
    }

    #[test]
    fn max_generation_retires_slot_instead_of_reusing_it() {
        let scheduler = GlobalScheduler::new();
        let state = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let entry = ScopeEntry {
            owner: WeakOwnerToken::from_typed(&state),
            parent: None,
            mode: OwnerMode::Transient,
        };
        let mut table = OwnerSlotTable {
            slots: vec![OwnerSlot {
                generation: u64::MAX,
                state: OwnerSlotState::Inactive(entry),
            }],
            free: Vec::new(),
            next_slot: 1,
        };
        table.release(OwnerId(0, u64::MAX));
        assert!(table.entry(OwnerId(0, u64::MAX)).is_none());
        assert_eq!(table.free_len(), 0);

        let replacement = ScopeState::new(OwnerId::initial(0), scheduler);
        let replacement_id = table
            .alloc(&replacement, None, OwnerMode::Transient)
            .expect("owner allocation");
        assert_eq!(replacement_id.0, 1);
    }
}
