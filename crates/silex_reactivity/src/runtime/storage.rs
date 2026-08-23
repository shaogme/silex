//! Typed payload slots and erased node behavior.

use super::scheduler::GlobalScheduler;
use crate::{
    ReactiveError, ReactiveResult,
    borrow::{BorrowCell, BorrowRef, BorrowRefMut, BorrowSite, SharedCell},
    error::{ErrorEvent, ErrorSlotRef, HandlerLease},
    root::{CleanupFailure, CloseError},
    unsafe_boundary::ScopedPtr,
};
use std::{
    cell::Cell,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    rc::Rc,
};

pub(crate) struct AllocationCounters {
    pub(crate) typed_slots: Cell<usize>,
    pub(crate) error_slots: Cell<usize>,
}

impl AllocationCounters {
    pub(crate) fn new() -> Self {
        Self {
            typed_slots: Cell::new(0),
            error_slots: Cell::new(0),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum AllocationKind {
    Typed,
    Error,
}

pub(crate) struct AllocationLease {
    counters: Rc<AllocationCounters>,
    kind: AllocationKind,
}

impl AllocationLease {
    pub(crate) fn new(counters: Rc<AllocationCounters>, kind: AllocationKind) -> Self {
        let count = match kind {
            AllocationKind::Typed => &counters.typed_slots,
            AllocationKind::Error => &counters.error_slots,
        };
        count.set(count.get().saturating_add(1));
        Self { counters, kind }
    }
}

impl Drop for AllocationLease {
    fn drop(&mut self) {
        let count = match self.kind {
            AllocationKind::Typed => &self.counters.typed_slots,
            AllocationKind::Error => &self.counters.error_slots,
        };
        count.set(count.get().saturating_sub(1));
    }
}

/// A value cell that never moves its contents out while user code runs.
pub(crate) struct LeaseCell<T> {
    value: BorrowCell<T>,
}

impl<T> LeaseCell<T> {
    pub(crate) fn new(value: T) -> Self {
        Self {
            value: BorrowCell::new(value, BorrowSite::Payload),
        }
    }

    pub(crate) fn try_read<'cell>(
        &'cell self,
        scheduler: SharedCell<GlobalScheduler>,
    ) -> ReactiveResult<ReadLease<'cell, T>> {
        let value = self
            .value
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let ticket = LeaseTicket::new(scheduler)?;
        Ok(ReadLease {
            value,
            _ticket: ticket,
        })
    }

    pub(crate) fn try_write<'cell>(
        &'cell self,
        scheduler: SharedCell<GlobalScheduler>,
    ) -> ReactiveResult<WriteLease<'cell, T>> {
        let value = self
            .value
            .try_write()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let ticket = LeaseTicket::new(scheduler)?;
        Ok(WriteLease {
            value,
            _ticket: ticket,
        })
    }

    pub(crate) fn try_peek<R>(&self, f: impl FnOnce(&T) -> R) -> ReactiveResult<Option<R>> {
        self.value
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)
            .map(|value| Some(f(&value)))
    }
}

impl<T> LeaseCell<Option<T>> {
    pub(crate) fn is_initialized(&self) -> ReactiveResult<bool> {
        Ok(self
            .value
            .try_read()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .is_some())
    }

    pub(crate) fn clear(&self) -> ReactiveResult<()> {
        self.value
            .try_write()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .take();
        Ok(())
    }
}

struct LeaseTicket {
    scheduler: SharedCell<GlobalScheduler>,
    close_reports: Rc<super::scheduler::CloseReportQueue>,
}

impl LeaseTicket {
    fn new(scheduler: SharedCell<GlobalScheduler>) -> ReactiveResult<Self> {
        let mut scheduler_ref = scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?;
        let close_reports = scheduler_ref.close_reports.clone();
        scheduler_ref.active_leases = scheduler_ref.active_leases.saturating_add(1);
        drop(scheduler_ref);
        Ok(Self {
            scheduler,
            close_reports,
        })
    }
}

impl Drop for LeaseTicket {
    fn drop(&mut self) {
        if let Ok(mut scheduler) = self.scheduler.try_borrow_mut() {
            scheduler.active_leases = scheduler.active_leases.saturating_sub(1);
        } else if let Some(error) =
            CloseError::from_failures(vec![CleanupFailure::Runtime(ReactiveError::BorrowConflict)])
        {
            self.close_reports.push(error);
        }
    }
}

pub(crate) struct ReadLease<'cell, T: ?Sized> {
    value: BorrowRef<'cell, T>,
    _ticket: LeaseTicket,
}

impl<T: ?Sized> Deref for ReadLease<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'cell, T: ?Sized> ReadLease<'cell, T> {
    pub(crate) fn try_map<U: ?Sized>(
        self,
        f: impl FnOnce(&T) -> Option<&U>,
    ) -> Result<ReadLease<'cell, U>, Self> {
        let ReadLease { value, _ticket } = self;
        match value.try_map(f) {
            Ok(value) => Ok(ReadLease { value, _ticket }),
            Err(value) => Err(ReadLease { value, _ticket }),
        }
    }
}

pub(crate) struct WriteLease<'cell, T: ?Sized> {
    value: BorrowRefMut<'cell, T>,
    _ticket: LeaseTicket,
}

impl<T: ?Sized> Deref for WriteLease<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: ?Sized> DerefMut for WriteLease<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<'cell, T: ?Sized> WriteLease<'cell, T> {
    pub(crate) fn try_map<U: ?Sized>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<WriteLease<'cell, U>, Self> {
        let WriteLease { value, _ticket } = self;
        match value.try_map(f) {
            Ok(value) => Ok(WriteLease { value, _ticket }),
            Err(value) => Err(WriteLease { value, _ticket }),
        }
    }
}

impl<'cell, T> ReadLease<'cell, Option<T>> {
    pub(crate) fn into_initialized(self) -> ReactiveResult<ReadLease<'cell, T>> {
        match self.try_map(Option::as_ref) {
            Ok(lease) => Ok(lease),
            Err(_lease) => Err(ReactiveError::NoSuchNode),
        }
    }
}

impl<'cell, T> WriteLease<'cell, Option<T>> {
    pub(crate) fn into_initialized(self) -> ReactiveResult<WriteLease<'cell, T>> {
        match self.try_map(Option::as_mut) {
            Ok(lease) => Ok(lease),
            Err(_lease) => Err(ReactiveError::NoSuchNode),
        }
    }
}

/// A typed payload slot whose allocation is owned by exactly one node.
pub(crate) struct TypedSlot<T> {
    value: LeaseCell<Option<T>>,
}

impl<T> TypedSlot<T> {
    pub(crate) fn clear(&self) -> ReactiveResult<()> {
        self.value.clear()
    }

    pub(crate) fn is_initialized(&self) -> ReactiveResult<bool> {
        self.value.is_initialized()
    }

    pub(crate) fn try_read<'scope>(
        &'scope self,
        scheduler: SharedCell<GlobalScheduler>,
    ) -> ReactiveResult<ReadLease<'scope, Option<T>>> {
        self.value.try_read(scheduler)
    }

    pub(crate) fn try_write<'scope>(
        &'scope self,
        scheduler: SharedCell<GlobalScheduler>,
    ) -> ReactiveResult<WriteLease<'scope, Option<T>>> {
        self.value.try_write(scheduler)
    }
}

pub(crate) struct TypedNodeRef<'scope, T> {
    slot: ScopedPtr<TypedSlot<T>>,
    marker: PhantomData<fn(&'scope ()) -> &'scope T>,
}

impl<T> Copy for TypedNodeRef<'_, T> {}

impl<T> Clone for TypedNodeRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'scope, T> TypedNodeRef<'scope, T> {
    pub(crate) fn from_pointer(pointer: ScopedPtr<()>) -> Self {
        Self {
            slot: pointer.cast(),
            marker: PhantomData,
        }
    }

    pub(crate) fn pointer(self) -> ScopedPtr<TypedSlot<T>> {
        self.slot
    }
}

pub(crate) struct TypedSlotAllocation<'scope, T> {
    slot: Option<Box<TypedSlot<T>>>,
    lease: Option<AllocationLease>,
    marker: PhantomData<fn(&'scope ()) -> &'scope T>,
}

impl<'scope, T> TypedSlotAllocation<'scope, T> {
    pub(crate) fn new(value: Option<T>, counters: Rc<AllocationCounters>) -> Self {
        let slot = Box::new(TypedSlot {
            value: LeaseCell::new(value),
        });
        Self {
            slot: Some(slot),
            lease: Some(AllocationLease::new(counters, AllocationKind::Typed)),
            marker: PhantomData,
        }
    }

    pub(crate) fn into_owned(mut self) -> ReactiveResult<OwnedTypedSlot<T>> {
        let slot = self.slot.take().ok_or(ReactiveError::InvariantViolation)?;
        let lease = self.lease.take().ok_or(ReactiveError::InvariantViolation)?;
        Ok(OwnedTypedSlot {
            slot,
            _lease: lease,
        })
    }
}

pub(crate) struct OwnedTypedSlot<T> {
    slot: Box<TypedSlot<T>>,
    _lease: AllocationLease,
}

impl<T> OwnedTypedSlot<T> {
    pub(crate) fn slot(&self) -> &TypedSlot<T> {
        self.slot.as_ref()
    }

    pub(crate) fn identity(&self) -> ScopedPtr<()> {
        ScopedPtr::from_ref(self.slot.as_ref()).cast()
    }
}

pub(crate) trait PayloadOwner {
    fn clear(&self) -> ReactiveResult<()>;
    fn identity(&self) -> ScopedPtr<()>;
}

struct SlotOwner<'scope, T> {
    slot: OwnedTypedSlot<T>,
    marker: PhantomData<fn() -> &'scope T>,
}

impl<T> PayloadOwner for SlotOwner<'_, T> {
    fn clear(&self) -> ReactiveResult<()> {
        self.slot.slot().clear()
    }

    fn identity(&self) -> ScopedPtr<()> {
        self.slot.identity()
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
        scheduler: SharedCell<GlobalScheduler>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>>;

    fn commit(&mut self, scheduler: SharedCell<GlobalScheduler>) -> ReactiveResult<()>;

    fn discard_pending(&mut self);

    fn has_value(&self) -> ReactiveResult<bool>;

    fn clear(&mut self) -> ReactiveResult<()>;

    fn value_slot_identity(&self) -> Option<ScopedPtr<()>>;

    fn error_slot_identity(&self) -> ScopedPtr<()>;
}

pub(crate) struct ComputationStorage<'scope> {
    pub(crate) computation: LeaseCell<Option<Box<dyn ComputationBehavior<'scope> + 'scope>>>,
    value_identity: Option<ScopedPtr<()>>,
    error_identity: ScopedPtr<()>,
}

impl<'scope> ComputationStorage<'scope> {
    pub(crate) fn new(
        computation: Box<dyn ComputationBehavior<'scope> + 'scope>,
    ) -> ReactiveResult<Self> {
        let computation = LeaseCell::new(Some(computation));
        let identities = computation.try_peek(|computation| {
            computation
                .as_ref()
                .map(|computation| {
                    (
                        computation.value_slot_identity(),
                        computation.error_slot_identity(),
                    )
                })
                .ok_or(ReactiveError::InvariantViolation)
        })?;
        let (value_identity, error_identity) =
            identities.ok_or(ReactiveError::InvariantViolation)??;
        Ok(Self {
            computation,
            value_identity,
            error_identity,
        })
    }
}

impl Drop for ComputationStorage<'_> {
    fn drop(&mut self) {
        if let Ok(mut computation) = self.computation.value.try_write()
            && let Some(computation) = computation.as_mut()
        {
            let _ = computation.clear();
        }
    }
}

pub(crate) enum NodeStorage<'scope> {
    Value(Box<dyn PayloadOwner + 'scope>),
    Computation(ComputationStorage<'scope>),
    Callback(Box<dyn PayloadOwner + 'scope>),
}

impl<'scope> NodeStorage<'scope> {
    pub(crate) fn value<T: 'scope>(slot: TypedSlotAllocation<'scope, T>) -> ReactiveResult<Self> {
        Ok(Self::Value(Box::new(SlotOwner {
            slot: slot.into_owned()?,
            marker: PhantomData,
        })))
    }

    pub(crate) fn callback<T: 'scope>(
        slot: TypedSlotAllocation<'scope, T>,
    ) -> ReactiveResult<Self> {
        Ok(Self::Callback(Box::new(SlotOwner {
            slot: slot.into_owned()?,
            marker: PhantomData,
        })))
    }

    pub(crate) fn payload_identity(&self) -> Option<ScopedPtr<()>> {
        match self {
            Self::Value(owner) | Self::Callback(owner) => Some(owner.identity()),
            Self::Computation(storage) => storage.value_identity,
        }
    }

    pub(crate) fn error_slot_identity(&self) -> Option<ScopedPtr<()>> {
        match self {
            Self::Computation(storage) => Some(storage.error_identity),
            Self::Value(_) | Self::Callback(_) => None,
        }
    }
}

impl Drop for NodeStorage<'_> {
    fn drop(&mut self) {
        match self {
            Self::Value(owner) | Self::Callback(owner) => {
                let _ = owner.clear();
            }
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
        let Some(callback) = self.callback.take() else {
            return Err(ErrorEvent::invariant("cleanup thunk"));
        };
        callback()
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
            SharedCell<GlobalScheduler>,
        )
            -> Result<ComputedEvaluation<T>, ComputationExecutionError<'scope>>
        + 'scope,
>;
pub(crate) type ChangePredicate<'scope, T> =
    Box<dyn for<'value> Fn(Option<&'value T>, &'value T) -> bool + 'scope>;

/// Shared evaluator/output-policy kernel for effects, previous effects,
/// computed values, and watchers.
pub(crate) struct ComputedNode<'scope, T, E> {
    slot: Option<Rc<OwnedTypedSlot<T>>>,
    error_slot: ErrorSlotRef<'scope, E>,
    evaluator: ComputedEvaluator<'scope, T>,
    changed: ChangePredicate<'scope, T>,
    notify: bool,
    pending: Option<T>,
    marker: PhantomData<fn() -> E>,
}

impl<'scope, T, E> ComputedNode<'scope, T, E> {
    pub(crate) fn new(
        slot: Option<TypedSlotAllocation<'scope, T>>,
        error_slot: ErrorSlotRef<'scope, E>,
        evaluator: ComputedEvaluator<'scope, T>,
        changed: ChangePredicate<'scope, T>,
        notify: bool,
    ) -> ReactiveResult<Self> {
        let slot = slot
            .map(TypedSlotAllocation::into_owned)
            .transpose()?
            .map(Rc::new);
        Ok(Self {
            slot,
            error_slot,
            evaluator,
            changed,
            notify,
            pending: None,
            marker: PhantomData,
        })
    }
}

impl<'scope, T, E> ComputationBehavior<'scope> for ComputedNode<'scope, T, E>
where
    T: 'scope,
    E: 'scope,
{
    fn execute(
        &mut self,
        scheduler: SharedCell<GlobalScheduler>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>> {
        let old = self
            .slot
            .as_ref()
            .map(|slot| slot.slot().try_read(scheduler.clone()))
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

    fn commit(&mut self, scheduler: SharedCell<GlobalScheduler>) -> ReactiveResult<()> {
        let Some(slot) = self.slot.as_ref() else {
            return Ok(());
        };
        let mut value = slot.slot().try_write(scheduler)?;
        *value = self.pending.take();
        Ok(())
    }

    fn discard_pending(&mut self) {
        self.pending = None;
    }

    fn has_value(&self) -> ReactiveResult<bool> {
        match self.slot.as_ref() {
            Some(slot) => slot.slot().is_initialized(),
            None => Ok(false),
        }
    }

    fn clear(&mut self) -> ReactiveResult<()> {
        self.pending = None;
        if let Some(slot) = self.slot.as_ref() {
            slot.slot().clear()?;
        }
        Ok(())
    }

    fn error_slot_identity(&self) -> ScopedPtr<()> {
        self.error_slot.identity()
    }

    fn value_slot_identity(&self) -> Option<ScopedPtr<()>> {
        self.slot.as_ref().map(|slot| slot.identity())
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
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .active_leases,
            2
        );
        assert!(matches!(
            cell.try_write(scheduler.clone()),
            Err(ReactiveError::BorrowConflict)
        ));
        drop(second);
        drop(first);
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .active_leases,
            0
        );
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
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .active_leases,
            0
        );
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
        assert_eq!(
            scheduler
                .try_borrow()
                .expect("scheduler read")
                .active_leases,
            0
        );
        assert_eq!(
            *cell.try_read(scheduler).expect("lease should be reusable"),
            1
        );
    }
}
