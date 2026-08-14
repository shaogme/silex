//! Global flat scheduler and scope bitmask lifetime tracking.

use super::model::ScopeState;
use crate::{ReactiveError, internal::RawId, unsafe_boundary::WeakOwnerToken};

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ScopeId(pub(crate) u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TargetNode {
    pub(crate) scope_id: ScopeId,
    pub(crate) node: RawId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Observer {
    pub(crate) scope_id: ScopeId,
    pub(crate) node: RawId,
}

#[derive(Clone)]
pub(crate) struct TrackingContext {
    pub(crate) observer: Option<ActiveObserver>,
    pub(crate) blocked_scopes: Vec<ScopeId>,
}

#[derive(Clone)]
pub(crate) struct ActiveObserver {
    pub(crate) scheduler: Rc<RefCell<GlobalScheduler>>,
    pub(crate) observer: Observer,
}

thread_local! {
    static ACTIVE_CONTEXT: RefCell<Vec<TrackingContext>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn active_ctx() -> Option<TrackingContext> {
    ACTIVE_CONTEXT.with(|stack| stack.borrow().last().cloned())
}

#[cfg(feature = "test-support")]
pub(crate) fn active_observer_for(scheduler: &Rc<RefCell<GlobalScheduler>>) -> Option<Observer> {
    active_ctx().and_then(|ctx| {
        ctx.observer
            .and_then(|active| Rc::ptr_eq(&active.scheduler, scheduler).then_some(active.observer))
    })
}

/// Restores the previous tracking ctx when the surrounding operation
/// unwinds. The ctx is process-local only; runtime state remains owned by
/// the explicit scheduler stored in each handle.
pub(crate) struct ObserverFrame {
    active: bool,
}

impl ObserverFrame {
    pub(crate) fn push(
        scheduler: Rc<RefCell<GlobalScheduler>>,
        observer: Option<Observer>,
    ) -> Self {
        ACTIVE_CONTEXT.with(|stack| {
            stack.borrow_mut().push(TrackingContext {
                observer: observer.map(|observer| ActiveObserver {
                    scheduler,
                    observer,
                }),
                blocked_scopes: Vec::new(),
            });
        });
        Self { active: true }
    }

    pub(crate) fn push_untracked(_scheduler: Rc<RefCell<GlobalScheduler>>) -> Self {
        ACTIVE_CONTEXT.with(|stack| {
            stack.borrow_mut().push(TrackingContext {
                observer: None,
                blocked_scopes: Vec::new(),
            });
        });
        Self { active: true }
    }

    pub(crate) fn push_child(scheduler: Rc<RefCell<GlobalScheduler>>, scope_id: ScopeId) -> Self {
        let inherited = active_ctx().and_then(|mut ctx| {
            let active = ctx.observer.take()?;
            if !Rc::ptr_eq(&active.scheduler, &scheduler) {
                return None;
            }
            ctx.blocked_scopes.push(scope_id);
            Some(TrackingContext {
                observer: Some(active),
                blocked_scopes: ctx.blocked_scopes,
            })
        });
        ACTIVE_CONTEXT.with(|stack| {
            stack
                .borrow_mut()
                .push(inherited.unwrap_or(TrackingContext {
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
        let mut scheduler = self
            .scheduler
            .try_borrow_mut()
            .expect("initial flush guard must release without a scheduler borrow");
        scheduler.initial_flush_depth = scheduler.initial_flush_depth.saturating_sub(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScheduledTask {
    pub(crate) scope_id: ScopeId,
    pub(crate) node: RawId,
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
}

pub(crate) struct GlobalScheduler {
    active_mask: BitSet,
    scopes: Vec<Option<ScopeEntry>>,
    next_scope_id: u32,
    free_scope_ids: Vec<u32>,
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
            next_scope_id: 0,
            free_scope_ids: Vec::new(),
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

    pub(crate) fn alloc_scope<'scope>(&mut self, state: &ScopeState<'scope>) -> ScopeId {
        let id = self.free_scope_ids.pop().unwrap_or_else(|| {
            let id = self.next_scope_id;
            self.next_scope_id = self.next_scope_id.wrapping_add(1);
            id
        });
        let scope_id = ScopeId(id);

        let owner = WeakOwnerToken::from_typed(state);

        self.active_mask.set(id, true);
        let index = id as usize;
        if index >= self.scopes.len() {
            self.scopes.resize_with(index + 1, || None);
        }
        self.scopes[index] = Some(ScopeEntry { owner });
        scope_id
    }

    pub(crate) fn deactivate_scope(&mut self, id: ScopeId) {
        if !self.is_scope_active(id) {
            return;
        }
        self.active_mask.set(id.0, false);
        self.global_queue.retain(|task| task.scope_id != id);
    }

    pub(crate) fn release_scope_id(&mut self, id: ScopeId) {
        if self.is_scope_active(id) || self.free_scope_ids.contains(&id.0) {
            return;
        }
        let index = id.0 as usize;
        if index < self.scopes.len() {
            self.scopes[index] = None;
        }
        self.free_scope_ids.push(id.0);
    }

    pub(crate) fn is_scope_active(&self, id: ScopeId) -> bool {
        self.active_mask.is_set(id.0)
    }

    pub(crate) fn is_scope_current(&self, id: ScopeId, expected: &WeakOwnerToken) -> bool {
        if !self.is_scope_active(id) {
            return false;
        }
        self.scopes
            .get(id.0 as usize)
            .and_then(Option::as_ref)
            .is_some_and(|entry| entry.owner.ptr_eq(expected))
    }

    pub(crate) fn active_scope_ids(&self) -> Vec<ScopeId> {
        self.scopes
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry
                    .as_ref()
                    .map(|_| ScopeId(index as u32))
                    .filter(|id| self.is_scope_active(*id))
            })
            .collect()
    }

    pub(crate) fn get_scope<'scope>(&self, id: ScopeId) -> Option<ScopeState<'scope>> {
        if !self.is_scope_active(id) {
            return None;
        }
        self.get_registered_scope(id)
    }

    pub(crate) fn get_scope_for_edge_cleanup<'scope>(
        &self,
        id: ScopeId,
    ) -> Option<ScopeState<'scope>> {
        self.get_registered_scope(id)
    }

    fn get_registered_scope<'scope>(&self, id: ScopeId) -> Option<ScopeState<'scope>> {
        let entry = self.scopes.get(id.0 as usize)?.as_ref()?;
        entry.owner.upgrade().map(|owner| owner.state())
    }

    pub(crate) fn enqueue_effect(&mut self, task: ScheduledTask) {
        if self.is_scope_active(task.scope_id) {
            self.global_queue.push_back(task);
        }
    }

    pub(crate) fn cancel_effect(&mut self, target: TargetNode) {
        self.global_queue
            .retain(|task| task.scope_id != target.scope_id || task.node != target.node);
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
    fn scope_slots_are_reused_only_after_release() {
        let scheduler = GlobalScheduler::new();
        let first = ScopeState::new(ScopeId(0), scheduler.clone());
        let first_id = scheduler.borrow_mut().alloc_scope(&first);
        first.borrow_mut().scope_id = first_id;
        scheduler.borrow_mut().deactivate_scope(first_id);
        assert!(
            scheduler
                .borrow()
                .get_scope_for_edge_cleanup(first_id)
                .is_some()
        );

        let second = ScopeState::new(ScopeId(0), scheduler.clone());
        let second_id = scheduler.borrow_mut().alloc_scope(&second);
        assert_ne!(first_id, second_id);

        scheduler.borrow_mut().release_scope_id(first_id);
        assert!(
            scheduler
                .borrow()
                .get_scope_for_edge_cleanup(first_id)
                .is_none()
        );

        let third = ScopeState::new(ScopeId(0), scheduler.clone());
        let third_id = scheduler.borrow_mut().alloc_scope(&third);

        assert_eq!(first_id, third_id);

        scheduler.borrow_mut().deactivate_scope(second_id);
        scheduler.borrow_mut().release_scope_id(second_id);
        scheduler.borrow_mut().deactivate_scope(third_id);
        scheduler.borrow_mut().release_scope_id(third_id);
    }
}
