//! Global scheduler, execution contexts, and scope lifetime tracking.

use super::model::ScopeState;
use crate::{
    ReactiveError, ReactiveResult,
    borrow::{BorrowCell, BorrowSite, SharedCell},
    internal::NodeId,
    root::CloseError,
    unsafe_boundary::{ActiveOwnerProof, CleanupOwnerProof, WeakOwnerToken},
};

use std::{cell::Cell, collections::VecDeque, rc::Rc};

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
pub(crate) struct OwnerId(pub(crate) u32, pub(crate) u32);

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
        let mut saw_context = false;
        for context in stack
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .iter()
            .rev()
        {
            saw_context = true;
            if Rc::ptr_eq(&context.scheduler, scheduler) {
                return Ok(());
            }
            if context.observer.is_some() {
                return Err(ReactiveError::RuntimeMismatch);
            }
        }
        if saw_context {
            Err(ReactiveError::RuntimeMismatch)
        } else {
            Ok(())
        }
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
            let observer = ctx.observer.take()?;
            ctx.blocked_scopes.push(owner_id);
            Some(ExecutionContext {
                scheduler: scheduler.clone(),
                observer: Some(observer),
                blocked_scopes: ctx.blocked_scopes,
            })
        });
        ACTIVE_CONTEXT.with(|stack| {
            stack
                .try_write()
                .map_err(|_| ReactiveError::BorrowConflict)?
                .push(inherited.unwrap_or(ExecutionContext {
                    scheduler,
                    observer: None,
                    blocked_scopes: Vec::new(),
                }));
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

pub(crate) struct BitSet {
    bits: Vec<u64>,
}

impl BitSet {
    pub(crate) fn new() -> Self {
        Self { bits: Vec::new() }
    }

    pub(crate) fn is_set(&self, id: u32) -> bool {
        let index = (id / 64) as usize;
        if index >= self.bits.len() {
            return false;
        }
        self.bits
            .get(index)
            .is_some_and(|bits| (bits & (1u64 << (id % 64))) != 0)
    }

    pub(crate) fn set(&mut self, id: u32, value: bool) {
        let index = (id / 64) as usize;
        if index >= self.bits.len() {
            if !value {
                return;
            }
            self.bits.resize(index.saturating_add(1), 0);
        }
        let bit = 1u64 << (id % 64);
        if let Some(bits) = self.bits.get_mut(index) {
            if value {
                *bits |= bit;
            } else {
                *bits &= !bit;
            }
        }
    }
}

pub(crate) struct ScopeEntry {
    owner: WeakOwnerToken,
    generation: u32,
    parent: Option<OwnerId>,
    mode: OwnerMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnerMode {
    Root,
    Persistent,
    Transient,
}

pub(crate) struct GlobalScheduler {
    active_mask: BitSet,
    scopes: Vec<Option<ScopeEntry>>,
    generations: Vec<u32>,
    next_owner_id: u32,
    free_owner_ids: Vec<u32>,
    epoch: u64,
    pub(crate) global_queue: VecDeque<ScheduledTask>,
    pub(crate) worklist: VecDeque<ScheduledTask>,
    pub(crate) running_queue: bool,
    pub(crate) batch_depth: usize,
    pub(crate) evaluating: usize,
    pub(crate) executing: usize,
    pub(crate) active_leases: usize,
    initial_flush_depth: usize,
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
                active_mask: BitSet::new(),
                scopes: Vec::new(),
                generations: Vec::new(),
                next_owner_id: 0,
                free_owner_ids: Vec::new(),
                epoch: 1,
                global_queue: VecDeque::new(),
                worklist: VecDeque::new(),
                running_queue: false,
                batch_depth: 0,
                evaluating: 0,
                executing: 0,
                active_leases: 0,
                initial_flush_depth: 0,
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
    ) -> OwnerId {
        let id = self.free_owner_ids.pop().unwrap_or_else(|| {
            let id = self.next_owner_id;
            self.next_owner_id = self.next_owner_id.wrapping_add(1);
            id
        });
        let generation = self.generations.get(id as usize).copied().unwrap_or(0);
        let owner_id = OwnerId(id, generation);

        let owner = WeakOwnerToken::from_typed(state);

        self.active_mask.set(id, true);
        let index = id as usize;
        if index >= self.scopes.len() {
            self.scopes.resize_with(index.saturating_add(1), || None);
        }
        if index >= self.generations.len() {
            self.generations.resize(index.saturating_add(1), 0);
        }
        if let Some(scope) = self.scopes.get_mut(index) {
            *scope = Some(ScopeEntry {
                owner,
                generation,
                parent,
                mode,
            });
        }
        owner_id
    }

    pub(crate) fn deactivate_scope(&mut self, id: OwnerId) {
        if !self.is_scope_active(id) {
            return;
        }
        self.active_mask.set(id.0, false);
        self.global_queue.retain(|task| task.owner_id != id);
        self.worklist.retain(|task| task.owner_id != id);
    }

    pub(crate) fn release_owner_id(&mut self, id: OwnerId) {
        if self.is_scope_active(id) || self.free_owner_ids.contains(&id.0) {
            return;
        }
        let index = id.0 as usize;
        let Some(entry) = self.scopes.get(index).and_then(Option::as_ref) else {
            return;
        };
        if entry.generation != id.1 {
            return;
        }
        if let Some(scope) = self.scopes.get_mut(index) {
            *scope = None;
        }
        if index >= self.generations.len() {
            self.generations.resize(index.saturating_add(1), 0);
        }
        if let Some(generation) = self.generations.get_mut(index) {
            *generation = id.1.wrapping_add(1);
        }
        self.free_owner_ids.push(id.0);
    }

    pub(crate) fn is_scope_active(&self, id: OwnerId) -> bool {
        self.active_mask.is_set(id.0)
            && self
                .scopes
                .get(id.0 as usize)
                .and_then(Option::as_ref)
                .is_some_and(|entry| entry.generation == id.1)
    }

    pub(crate) fn is_scope_current(&self, id: OwnerId, expected: &WeakOwnerToken) -> bool {
        if !self.is_scope_active(id) {
            return false;
        }
        self.scopes
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .is_some_and(|entry| entry.generation == id.1 && entry.owner.ptr_eq(expected))
    }

    /// Resolve an active owner proof after validating the complete registry
    /// identity and active phase.
    pub(crate) fn resolve_active_owner<'scope>(
        &self,
        id: OwnerId,
        expected: &WeakOwnerToken,
    ) -> ReactiveResult<Option<ActiveOwnerProof<'scope>>> {
        let Some(entry) = self.scopes.get(id.0 as usize).and_then(Option::as_ref) else {
            return Ok(None);
        };
        if !self.is_scope_active(id) || !entry.owner.ptr_eq(expected) {
            return Ok(None);
        }
        let Some(state) = entry.owner.upgrade_erased() else {
            return Ok(None);
        };
        ActiveOwnerProof::from_registry(id, entry.generation, expected, state)
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn active_owner_ids(&self) -> Vec<OwnerId> {
        self.scopes
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry
                    .as_ref()
                    .map(|entry| OwnerId(index as u32, entry.generation))
                    .filter(|id| self.is_scope_active(*id))
            })
            .collect()
    }

    pub(crate) fn get_scope<'scope>(
        &self,
        id: OwnerId,
    ) -> ReactiveResult<Option<ScopeState<'scope>>> {
        let Some(entry) = self.scopes.get(id.0 as usize).and_then(Option::as_ref) else {
            return Ok(None);
        };
        if !self.is_scope_active(id) {
            return Ok(None);
        }
        let Some(state) = entry.owner.upgrade_erased() else {
            return Ok(None);
        };
        ActiveOwnerProof::from_registry(id, entry.generation, &entry.owner, state)
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
        let Some(entry) = self.scopes.get(id.0 as usize).and_then(Option::as_ref) else {
            return Ok(None);
        };
        if entry.generation != id.1 {
            return Ok(None);
        }
        let Some(state) = entry.owner.upgrade_erased() else {
            return Ok(None);
        };
        CleanupOwnerProof::from_registry(id, entry.generation, &entry.owner, state)
    }

    pub(crate) fn owner_metadata(&self, id: OwnerId) -> Option<(OwnerMode, Option<OwnerId>)> {
        self.scopes
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .filter(|entry| entry.generation == id.1)
            .map(|entry| (entry.mode, entry.parent))
    }

    pub(crate) fn enqueue_effect(&mut self, task: ScheduledTask) {
        if self.is_scope_active(task.owner_id) {
            self.global_queue.push_back(task);
        }
    }

    pub(crate) fn cancel_effect(&mut self, target: TargetNode) {
        self.global_queue
            .retain(|task| task.owner_id != target.owner_id || task.node != target.node);
        self.worklist
            .retain(|task| task.owner_id != target.owner_id || task.node != target.node);
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.batch_depth == 0 && self.evaluating == 0 && self.executing == 0 && !self.running_queue
    }

    pub(crate) fn should_flush(&self) -> bool {
        self.initial_flush_depth == 0
            && self.is_idle()
            && self.active_leases == 0
            && (!self.global_queue.is_empty() || !self.worklist.is_empty())
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
            .alloc_owner(&first, None, OwnerMode::Transient);
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
            .alloc_owner(&second, None, OwnerMode::Transient);
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
            .alloc_owner(&third, None, OwnerMode::Transient);

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
}
