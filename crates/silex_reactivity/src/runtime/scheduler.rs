//! Global scheduler, execution contexts, and scope lifetime tracking.

use super::model::{ScopePhase, ScopeState};
use crate::{
    ReactiveError,
    internal::NodeId,
    unsafe_boundary::{OwnerToken, WeakOwnerToken},
};

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

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
    pub(crate) scheduler: Rc<RefCell<GlobalScheduler>>,
    pub(crate) observer: Option<Observer>,
    pub(crate) blocked_scopes: Vec<OwnerId>,
}

thread_local! {
    static ACTIVE_CONTEXT: RefCell<Vec<ExecutionContext>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn validate_active_scheduler(
    scheduler: &Rc<RefCell<GlobalScheduler>>,
) -> Result<(), ReactiveError> {
    ACTIVE_CONTEXT.with(|stack| {
        let mut saw_context = false;
        for context in stack.borrow().iter().rev() {
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
pub(crate) fn active_ctx(scheduler: &Rc<RefCell<GlobalScheduler>>) -> Option<ExecutionContext> {
    ACTIVE_CONTEXT.with(|stack| {
        stack.borrow().iter().rev().find_map(|context| {
            if Rc::ptr_eq(&context.scheduler, scheduler) || context.observer.is_some() {
                Some(context.clone())
            } else {
                None
            }
        })
    })
}

#[cfg(feature = "test-support")]
pub(crate) fn active_observer_for(scheduler: &Rc<RefCell<GlobalScheduler>>) -> Option<Observer> {
    active_ctx(scheduler).and_then(|ctx| {
        ctx.observer
            .filter(|_| Rc::ptr_eq(&ctx.scheduler, scheduler))
    })
}

/// Restores the previous execution context when the surrounding operation
/// unwinds. The stack is thread-local, but every frame is owned by one
/// scheduler and runtime state remains owned by that scheduler.
pub(crate) struct ObserverFrame {
    active: bool,
}

impl ObserverFrame {
    pub(crate) fn push(
        scheduler: Rc<RefCell<GlobalScheduler>>,
        observer: Option<Observer>,
    ) -> Self {
        ACTIVE_CONTEXT.with(|stack| {
            stack.borrow_mut().push(ExecutionContext {
                scheduler,
                observer,
                blocked_scopes: Vec::new(),
            });
        });
        Self { active: true }
    }

    pub(crate) fn push_untracked(scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        ACTIVE_CONTEXT.with(|stack| {
            stack.borrow_mut().push(ExecutionContext {
                scheduler,
                observer: None,
                blocked_scopes: Vec::new(),
            });
        });
        Self { active: true }
    }

    pub(crate) fn push_child(scheduler: Rc<RefCell<GlobalScheduler>>, owner_id: OwnerId) -> Self {
        let inherited = active_ctx(&scheduler).and_then(|mut ctx| {
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
                .borrow_mut()
                .push(inherited.unwrap_or(ExecutionContext {
                    scheduler,
                    observer: None,
                    blocked_scopes: Vec::new(),
                }));
        });
        Self { active: true }
    }
}

impl Drop for ObserverFrame {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        ACTIVE_CONTEXT.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert!(popped.is_some(), "tracking ctx stack underflow");
        });
    }
}

/// Prevents queue flushing while a computation is still provisional.
///
/// Initial callbacks may register children and cleanups that can write to
/// other signals while the provisional node is being rolled back. A nested
/// guard keeps that invariant intact for computations created by the callback.
pub(crate) struct InitialFlushGuard {
    scheduler: Rc<RefCell<GlobalScheduler>>,
}

impl InitialFlushGuard {
    pub(crate) fn try_new(scheduler: Rc<RefCell<GlobalScheduler>>) -> Result<Self, ReactiveError> {
        scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .initial_flush_depth += 1;
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
        (self.bits[index] & (1u64 << (id % 64))) != 0
    }

    pub(crate) fn set(&mut self, id: u32, value: bool) {
        let index = (id / 64) as usize;
        if index >= self.bits.len() {
            if !value {
                return;
            }
            self.bits.resize(index + 1, 0);
        }
        let bit = 1u64 << (id % 64);
        if value {
            self.bits[index] |= bit;
        } else {
            self.bits[index] &= !bit;
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
    pub(crate) running_queue: bool,
    pub(crate) batch_depth: usize,
    pub(crate) evaluating: usize,
    pub(crate) executing: usize,
    pub(crate) active_leases: usize,
    initial_flush_depth: usize,
}

impl GlobalScheduler {
    pub(crate) fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            active_mask: BitSet::new(),
            scopes: Vec::new(),
            generations: Vec::new(),
            next_owner_id: 0,
            free_owner_ids: Vec::new(),
            epoch: 1,
            global_queue: VecDeque::new(),
            running_queue: false,
            batch_depth: 0,
            evaluating: 0,
            executing: 0,
            active_leases: 0,
            initial_flush_depth: 0,
        }))
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
            self.scopes.resize_with(index + 1, || None);
        }
        if index >= self.generations.len() {
            self.generations.resize(index + 1, 0);
        }
        self.scopes[index] = Some(ScopeEntry {
            owner,
            generation,
            parent,
            mode,
        });
        owner_id
    }

    pub(crate) fn deactivate_scope(&mut self, id: OwnerId) {
        if !self.is_scope_active(id) {
            return;
        }
        self.active_mask.set(id.0, false);
        self.global_queue.retain(|task| task.owner_id != id);
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
        if index < self.scopes.len() {
            self.scopes[index] = None;
        }
        if index >= self.generations.len() {
            self.generations.resize(index + 1, 0);
        }
        self.generations[index] = id.1.wrapping_add(1);
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

    /// Resolve a typed owner only after validating both the generational slot
    /// and the exact weak-state identity registered in that slot.
    pub(crate) fn resolve_owner<'scope>(
        &self,
        id: OwnerId,
        expected: &WeakOwnerToken,
    ) -> Option<OwnerToken<'scope>> {
        if !self.is_scope_current(id, expected) {
            return None;
        }
        let state = expected.upgrade_erased()?;
        if state
            .try_borrow()
            .ok()
            .is_none_or(|state| state.phase != ScopePhase::Active)
        {
            return None;
        }
        // SAFETY: `is_scope_current` checked the scheduler family through the
        // exact weak identity, the owner slot, and its generation. The state
        // remains registered until the close transaction has detached all
        // edges and cleared all typed payload slots.
        Some(unsafe { OwnerToken::from_validated(state) })
    }

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

    pub(crate) fn get_scope<'scope>(&self, id: OwnerId) -> Option<ScopeState<'scope>> {
        if !self.is_scope_active(id) {
            return None;
        }
        self.get_registered_scope(id)
    }

    pub(crate) fn get_scope_for_edge_cleanup<'scope>(
        &self,
        id: OwnerId,
    ) -> Option<ScopeState<'scope>> {
        self.get_registered_scope(id)
    }

    fn get_registered_scope<'scope>(&self, id: OwnerId) -> Option<ScopeState<'scope>> {
        let entry = self.scopes.get(id.0 as usize)?.as_ref()?;
        if entry.generation != id.1 {
            return None;
        }
        entry
            .owner
            .upgrade_erased()
            .map(|state| unsafe { OwnerToken::from_validated(state).state() })
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
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.batch_depth == 0 && self.evaluating == 0 && self.executing == 0 && !self.running_queue
    }

    pub(crate) fn should_flush(&self) -> bool {
        self.initial_flush_depth == 0
            && self.is_idle()
            && self.active_leases == 0
            && !self.global_queue.is_empty()
    }
}

#[cfg(test)]
mod tests {
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
        );
        let untracked = ObserverFrame::push_untracked(untracked_scheduler.clone());

        assert!(active_ctx(&tracked_scheduler).is_some_and(|context| context.observer.is_some()));
        assert!(active_ctx(&untracked_scheduler).is_some_and(|context| context.observer.is_none()));

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
        );

        let context = active_ctx(&source_scheduler).expect("foreign observer context");
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
            .borrow_mut()
            .alloc_owner(&first, None, OwnerMode::Transient);
        first.borrow_mut().owner_id = first_id;
        scheduler.borrow_mut().deactivate_scope(first_id);
        assert!(
            scheduler
                .borrow()
                .get_scope_for_edge_cleanup(first_id)
                .is_some()
        );

        let second = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let second_id = scheduler
            .borrow_mut()
            .alloc_owner(&second, None, OwnerMode::Transient);
        assert_ne!(first_id, second_id);

        scheduler.borrow_mut().release_owner_id(first_id);
        assert!(
            scheduler
                .borrow()
                .get_scope_for_edge_cleanup(first_id)
                .is_none()
        );

        let third = ScopeState::new(OwnerId::initial(0), scheduler.clone());
        let third_id = scheduler
            .borrow_mut()
            .alloc_owner(&third, None, OwnerMode::Transient);

        assert_eq!(first_id.0, third_id.0);
        assert_ne!(first_id.1, third_id.1);

        scheduler.borrow_mut().deactivate_scope(second_id);
        scheduler.borrow_mut().release_owner_id(second_id);
        scheduler.borrow_mut().deactivate_scope(third_id);
        scheduler.borrow_mut().release_owner_id(third_id);
    }
}
