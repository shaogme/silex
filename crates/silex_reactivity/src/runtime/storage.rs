//! Typed payload slots and erased node behavior.

use super::scheduler::GlobalScheduler;
use crate::{
    ReactiveError, ReactiveResult,
    error::{ErrorEvent, HandlerLease},
};
use std::{
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
};

/// A value cell that never moves its contents out while user code runs.
pub(crate) struct LeaseCell<T> {
    value: RefCell<T>,
}

impl<T> LeaseCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: RefCell::new(value),
        }
    }

    pub(crate) fn try_read<'cell>(
        &'cell self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> ReactiveResult<ReadLease<'cell, T>> {
        let value = self
            .value
            .try_borrow()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let ticket = LeaseTicket::new(scheduler)?;
        Ok(ReadLease {
            value,
            _ticket: ticket,
        })
    }

    pub(crate) fn try_write<'cell>(
        &'cell self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> ReactiveResult<WriteLease<'cell, T>> {
        let value = self
            .value
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let ticket = LeaseTicket::new(scheduler)?;
        Ok(WriteLease {
            value,
            _ticket: ticket,
        })
    }

    pub(crate) fn try_peek<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.value.try_borrow().ok().map(|value| f(&value))
    }
}

impl<T> LeaseCell<Option<T>> {
    pub(crate) fn is_initialized(&self) -> bool {
        self.value.try_borrow().is_ok_and(|value| value.is_some())
    }

    pub(crate) fn clear(&self) {
        let _ = self.value.borrow_mut().take();
    }
}

struct LeaseTicket {
    scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
}

impl LeaseTicket {
    fn new(scheduler: std::rc::Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<Self> {
        scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .active_leases += 1;
        Ok(Self { scheduler })
    }
}

impl Drop for LeaseTicket {
    fn drop(&mut self) {
        if let Ok(mut scheduler) = self.scheduler.try_borrow_mut() {
            scheduler.active_leases = scheduler.active_leases.saturating_sub(1);
        }
    }
}

pub(crate) struct ReadLease<'cell, T> {
    value: Ref<'cell, T>,
    _ticket: LeaseTicket,
}

impl<T> Deref for ReadLease<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pub(crate) struct WriteLease<'cell, T> {
    value: RefMut<'cell, T>,
    _ticket: LeaseTicket,
}

impl<T> Deref for WriteLease<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for WriteLease<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

/// A scope-local typed payload. The slot itself lives for the scope lifetime;
/// disposal clears the `Option<T>` and therefore drops the payload immediately.
pub(crate) struct TypedSlot<T> {
    value: LeaseCell<Option<T>>,
}

impl<T> TypedSlot<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: LeaseCell::new(Some(value)),
        }
    }

    pub(crate) fn empty() -> Self {
        Self {
            value: LeaseCell::new(None),
        }
    }

    pub(crate) fn clear(&self) {
        self.value.clear();
    }

    pub(crate) fn is_initialized(&self) -> bool {
        self.value.is_initialized()
    }

    pub(crate) fn try_read<'scope>(
        &'scope self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> ReactiveResult<ReadLease<'scope, Option<T>>> {
        self.value.try_read(scheduler)
    }

    pub(crate) fn try_write<'scope>(
        &'scope self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> ReactiveResult<WriteLease<'scope, Option<T>>> {
        self.value.try_write(scheduler)
    }
}

pub(crate) struct TypedNodeRef<'scope, T> {
    slot: &'scope TypedSlot<T>,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for TypedNodeRef<'_, T> {}

impl<T> Clone for TypedNodeRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> TypedNodeRef<'scope, T> {
    pub(crate) fn from_slot(slot: &'scope TypedSlot<T>) -> Self {
        Self {
            slot,
            marker: PhantomData,
        }
    }

    pub(crate) fn slot(&self) -> &'scope TypedSlot<T> {
        self.slot
    }
}

pub(crate) trait PayloadOwner {
    fn clear(&self);
}

struct SlotOwner<'scope, T> {
    slot: &'scope TypedSlot<T>,
}

impl<T> PayloadOwner for SlotOwner<'_, T> {
    fn clear(&self) {
        self.slot.clear();
    }
}

pub(crate) struct ComputationOutcome {
    pub(crate) commit_value: bool,
    pub(crate) notify: bool,
    pub(crate) stop_after_run: bool,
}

pub(crate) enum ComputationExecutionError<'scope> {
    Runtime(ReactiveError),
    Callback(ErrorEvent<'scope>),
}

pub(crate) trait ComputationBehavior<'scope> {
    fn execute(
        &mut self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>>;

    fn commit(&mut self, scheduler: std::rc::Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<()>;

    fn discard_pending(&mut self);

    fn has_value(&self) -> bool;

    fn clear(&mut self);
}

pub(crate) struct ComputationStorage<'scope> {
    pub(crate) computation: LeaseCell<Option<Box<dyn ComputationBehavior<'scope> + 'scope>>>,
}

impl<'scope> ComputationStorage<'scope> {
    pub(crate) fn new(computation: Box<dyn ComputationBehavior<'scope> + 'scope>) -> Self {
        Self {
            computation: LeaseCell::new(Some(computation)),
        }
    }
}

impl Drop for ComputationStorage<'_> {
    fn drop(&mut self) {
        if let Some(computation) = self.computation.value.get_mut().as_mut() {
            computation.clear();
        }
    }
}

pub(crate) enum NodeStorage<'scope> {
    Value(Box<dyn PayloadOwner + 'scope>),
    Computation(ComputationStorage<'scope>),
    Callback(Box<dyn PayloadOwner + 'scope>),
}

impl<'scope> NodeStorage<'scope> {
    pub(crate) fn value<T>(slot: &'scope TypedSlot<T>) -> Self {
        Self::Value(Box::new(SlotOwner { slot }))
    }

    pub(crate) fn callback<T>(slot: &'scope TypedSlot<T>) -> Self {
        Self::Callback(Box::new(SlotOwner { slot }))
    }
}

impl Drop for NodeStorage<'_> {
    fn drop(&mut self) {
        match self {
            Self::Value(owner) | Self::Callback(owner) => owner.clear(),
            Self::Computation(_) => {}
        }
    }
}

pub(crate) enum CallbackThunkError<E> {
    Runtime(ReactiveError),
    User(E),
}

pub(crate) type ThunkCallback<'scope, T, E> = Box<dyn FnMut(T) -> Result<(), E> + 'scope>;

pub(crate) struct CallbackThunk<'scope, T, E> {
    callback: ThunkCallback<'scope, T, E>,
}

impl<'scope, T, E> CallbackThunk<'scope, T, E> {
    pub(crate) fn new<F>(callback: F) -> Self
    where
        F: FnMut(T) -> Result<(), E> + 'scope,
    {
        Self {
            callback: Box::new(callback),
        }
    }

    pub(crate) fn call(&mut self, arg: T) -> Result<(), E> {
        (self.callback)(arg)
    }
}

pub(crate) type CleanupCallback<'scope> =
    Box<dyn FnOnce() -> Result<(), ErrorEvent<'scope>> + 'scope>;

pub(crate) struct CleanupThunk<'scope> {
    callback: Option<CleanupCallback<'scope>>,
}

impl<'scope> CleanupThunk<'scope> {
    pub(crate) fn new<E, F>(callback: F, handler: HandlerLease<'scope, E>) -> Self
    where
        E: 'scope,
        F: FnOnce() -> Result<(), E> + 'scope,
    {
        Self {
            callback: Some(Box::new(move || {
                callback().map_err(|error| ErrorEvent::deferred(error, &handler))
            })),
        }
    }

    pub(crate) fn call(mut self) -> Result<(), ErrorEvent<'scope>> {
        self.callback.take().expect("cleanup thunk called twice")()
    }
}

/// Result produced by one unified computation evaluator.
pub(crate) struct ComputedEvaluation<T> {
    pub(crate) value: T,
    pub(crate) stop_after_run: bool,
}

pub(crate) type ComputedEvaluator<'scope, T> = Box<
    dyn for<'value> FnMut(
            Option<&T>,
            Rc<RefCell<GlobalScheduler>>,
        )
            -> Result<ComputedEvaluation<T>, ComputationExecutionError<'scope>>
        + 'scope,
>;
pub(crate) type ChangePredicate<'scope, T> =
    Box<dyn for<'value> Fn(Option<&'value T>, &'value T) -> bool + 'scope>;

/// Shared evaluator/output-policy kernel for effects, previous effects,
/// computed values, and watchers.
pub(crate) struct ComputedNode<'scope, T, E> {
    slot: Option<&'scope TypedSlot<T>>,
    evaluator: ComputedEvaluator<'scope, T>,
    changed: ChangePredicate<'scope, T>,
    notify: bool,
    pending: Option<T>,
    marker: PhantomData<fn() -> E>,
}

impl<'scope, T, E> ComputedNode<'scope, T, E> {
    pub(crate) fn new(
        slot: Option<&'scope TypedSlot<T>>,
        evaluator: ComputedEvaluator<'scope, T>,
        changed: ChangePredicate<'scope, T>,
        notify: bool,
    ) -> Self {
        Self {
            slot,
            evaluator,
            changed,
            notify,
            pending: None,
            marker: PhantomData,
        }
    }
}

impl<'scope, T, E> ComputationBehavior<'scope> for ComputedNode<'scope, T, E>
where
    T: 'scope,
    E: 'scope,
{
    fn execute(
        &mut self,
        scheduler: Rc<RefCell<GlobalScheduler>>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>> {
        let old = self
            .slot
            .map(|slot| slot.try_read(scheduler.clone()))
            .transpose()
            .map_err(ComputationExecutionError::Runtime)?;
        let old_value = old.as_ref().and_then(|lease| lease.deref().as_ref());
        let evaluation = (self.evaluator)(old_value, scheduler)?;
        let changed = (self.changed)(old_value, &evaluation.value);
        drop(old);
        if changed && self.slot.is_some() {
            self.pending = Some(evaluation.value);
        }
        Ok(ComputationOutcome {
            commit_value: changed && self.slot.is_some(),
            notify: changed && self.notify,
            stop_after_run: evaluation.stop_after_run,
        })
    }

    fn commit(&mut self, scheduler: Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<()> {
        let Some(slot) = self.slot else {
            return Ok(());
        };
        let mut value = slot.try_write(scheduler)?;
        *value = self.pending.take();
        Ok(())
    }

    fn discard_pending(&mut self) {
        self.pending = None;
    }

    fn has_value(&self) -> bool {
        self.slot.is_some_and(TypedSlot::is_initialized)
    }

    fn clear(&mut self) {
        self.pending = None;
        if let Some(slot) = self.slot {
            slot.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn shared_read_leases_are_allowed() {
        let scheduler = GlobalScheduler::new();
        let cell = LeaseCell::new(1_i32);
        let first = cell
            .try_read(scheduler.clone())
            .expect("first read lease should succeed");
        let second = cell
            .try_read(scheduler.clone())
            .expect("shared read lease should succeed");
        assert_eq!(*first, 1);
        assert_eq!(*second, 1);
        assert_eq!(scheduler.borrow().active_leases, 2);
        assert!(matches!(
            cell.try_write(scheduler.clone()),
            Err(ReactiveError::BorrowConflict)
        ));
        drop(second);
        drop(first);
        assert_eq!(scheduler.borrow().active_leases, 0);
    }

    #[test]
    fn write_leases_conflict_with_reads_and_writes() {
        let scheduler = GlobalScheduler::new();
        let cell = LeaseCell::new(1_i32);
        let mut write = cell
            .try_write(scheduler.clone())
            .expect("write lease should succeed");
        *write += 1;
        assert!(matches!(
            cell.try_read(scheduler.clone()),
            Err(ReactiveError::BorrowConflict)
        ));
        assert!(matches!(
            cell.try_write(scheduler.clone()),
            Err(ReactiveError::BorrowConflict)
        ));
        drop(write);
        assert_eq!(scheduler.borrow().active_leases, 0);
        assert_eq!(*cell.try_read(scheduler).expect("read should succeed"), 2);
    }

    #[test]
    fn panic_drops_the_lease_ticket() {
        let scheduler = GlobalScheduler::new();
        let cell = LeaseCell::new(1_i32);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _lease = cell
                .try_write(scheduler.clone())
                .expect("write lease should succeed");
            panic!("lease body panic");
        }));
        assert!(panic.is_err());
        assert_eq!(scheduler.borrow().active_leases, 0);
        assert_eq!(
            *cell.try_read(scheduler).expect("lease should be reusable"),
            1
        );
    }
}
