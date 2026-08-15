//! Typed payload slots and erased node behavior.

use super::scheduler::{GlobalScheduler, ObserverFrame};
use crate::{
    ReactiveError, ReactiveResult,
    error::{ErrorEvent, ErrorSlot, HandlerLease},
};
use std::{
    cell::{Ref, RefCell, RefMut},
    marker::PhantomData,
    ops::{Deref, DerefMut},
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

pub(crate) type EffectCallback<'scope> =
    Box<dyn FnMut() -> Result<(), ErrorEvent<'scope>> + 'scope>;

pub(crate) struct EffectBehavior<'scope> {
    callback: EffectCallback<'scope>,
}

impl<'scope> EffectBehavior<'scope> {
    pub(crate) fn new<E, F>(
        callback: F,
        handler: HandlerLease<'scope, E>,
        slot: &'scope ErrorSlot<E>,
    ) -> Self
    where
        E: 'scope,
        F: FnMut() -> Result<(), E> + 'scope,
    {
        let mut callback = callback;
        Self {
            callback: Box::new(move || {
                callback().map_err(|error| ErrorEvent::new(error, &handler, slot))
            }),
        }
    }
}

impl<'scope> ComputationBehavior<'scope> for EffectBehavior<'scope> {
    fn execute(
        &mut self,
        _scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>> {
        (self.callback)().map_err(ComputationExecutionError::Callback)?;
        Ok(ComputationOutcome {
            commit_value: false,
            notify: false,
            stop_after_run: false,
        })
    }

    fn commit(&mut self, _scheduler: std::rc::Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<()> {
        Ok(())
    }

    fn discard_pending(&mut self) {}

    fn has_value(&self) -> bool {
        false
    }

    fn clear(&mut self) {}
}

pub(crate) type ValueComputeCallback<'scope, T, E> =
    Box<dyn FnMut(Option<&T>) -> Result<T, E> + 'scope>;

pub(crate) struct PreviousBehavior<'scope, T, E> {
    slot: &'scope TypedSlot<T>,
    callback: ValueComputeCallback<'scope, T, E>,
    pending: Option<T>,
    handler: HandlerLease<'scope, E>,
    errors: &'scope ErrorSlot<E>,
}

impl<'scope, T, E> PreviousBehavior<'scope, T, E> {
    pub(crate) fn new<F>(
        slot: &'scope TypedSlot<T>,
        callback: F,
        handler: HandlerLease<'scope, E>,
        errors: &'scope ErrorSlot<E>,
    ) -> Self
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        Self {
            slot,
            callback: Box::new(callback),
            pending: None,
            handler,
            errors,
        }
    }
}

impl<'scope, T, E> ComputationBehavior<'scope> for PreviousBehavior<'scope, T, E>
where
    T: 'scope,
    E: 'scope,
{
    fn execute(
        &mut self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>> {
        let old = self
            .slot
            .try_read(scheduler)
            .map_err(ComputationExecutionError::Runtime)?;
        let result = (self.callback)(old.as_ref());
        drop(old);
        self.pending = Some(result.map_err(|error| {
            ComputationExecutionError::Callback(ErrorEvent::new(error, &self.handler, self.errors))
        })?);
        Ok(ComputationOutcome {
            commit_value: true,
            notify: false,
            stop_after_run: false,
        })
    }

    fn commit(&mut self, scheduler: std::rc::Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<()> {
        let mut value = self.slot.try_write(scheduler)?;
        *value = self.pending.take();
        Ok(())
    }

    fn discard_pending(&mut self) {
        self.pending = None;
    }

    fn has_value(&self) -> bool {
        self.slot.is_initialized()
    }

    fn clear(&mut self) {
        self.pending = None;
        self.slot.clear();
    }
}

pub(crate) type WatchGetterCallback<'scope, T, E> = Box<dyn FnMut() -> Result<T, E> + 'scope>;
pub(crate) type WatchActionCallback<'scope, T, E> =
    Box<dyn FnMut(&T, Option<&T>) -> Result<(), E> + 'scope>;

pub(crate) struct WatchBehavior<'scope, T, E> {
    slot: &'scope TypedSlot<T>,
    getter: WatchGetterCallback<'scope, T, E>,
    callback: WatchActionCallback<'scope, T, E>,
    pending: Option<T>,
    initialized: bool,
    immediate: bool,
    once: bool,
    handler: HandlerLease<'scope, E>,
    errors: &'scope ErrorSlot<E>,
}

impl<'scope, T, E> WatchBehavior<'scope, T, E> {
    pub(crate) fn new<G, C>(
        slot: &'scope TypedSlot<T>,
        getter: G,
        callback: C,
        handler: HandlerLease<'scope, E>,
        errors: &'scope ErrorSlot<E>,
        immediate: bool,
        once: bool,
    ) -> Self
    where
        T: 'scope,
        E: 'scope,
        G: FnMut() -> Result<T, E> + 'scope,
        C: FnMut(&T, Option<&T>) -> Result<(), E> + 'scope,
    {
        Self {
            slot,
            getter: Box::new(getter),
            callback: Box::new(callback),
            pending: None,
            initialized: false,
            immediate,
            once,
            handler,
            errors,
        }
    }
}

impl<'scope, T, E> ComputationBehavior<'scope> for WatchBehavior<'scope, T, E>
where
    T: PartialEq + 'scope,
    E: 'scope,
{
    fn execute(
        &mut self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>> {
        let old = self
            .slot
            .try_read(scheduler.clone())
            .map_err(ComputationExecutionError::Runtime)?;
        let new = (self.getter)().map_err(|error| {
            ComputationExecutionError::Callback(ErrorEvent::new(error, &self.handler, self.errors))
        })?;
        let first_run = !self.initialized;
        let changed = first_run || old.as_ref().is_none_or(|value| *value != new);
        let should_callback = if first_run { self.immediate } else { changed };
        if should_callback {
            let callback_result = {
                let _observer_frame = ObserverFrame::push_untracked(scheduler.clone());
                (self.callback)(&new, old.as_ref())
            };
            callback_result.map_err(|error| {
                ComputationExecutionError::Callback(ErrorEvent::new(
                    error,
                    &self.handler,
                    self.errors,
                ))
            })?;
        }
        drop(old);
        if first_run || changed {
            self.pending = Some(new);
        }
        Ok(ComputationOutcome {
            commit_value: first_run || changed,
            notify: false,
            stop_after_run: should_callback && self.once,
        })
    }

    fn commit(&mut self, scheduler: std::rc::Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<()> {
        let mut value = self.slot.try_write(scheduler)?;
        *value = self.pending.take();
        self.initialized = true;
        Ok(())
    }

    fn discard_pending(&mut self) {
        self.pending = None;
    }

    fn has_value(&self) -> bool {
        self.slot.is_initialized()
    }

    fn clear(&mut self) {
        self.pending = None;
        self.slot.clear();
    }
}

pub(crate) struct MemoBehavior<'scope, T, E> {
    slot: &'scope TypedSlot<T>,
    callback: ValueComputeCallback<'scope, T, E>,
    pending: Option<T>,
    handler: HandlerLease<'scope, E>,
    errors: &'scope ErrorSlot<E>,
}

impl<'scope, T, E> MemoBehavior<'scope, T, E> {
    pub(crate) fn new<F>(
        slot: &'scope TypedSlot<T>,
        callback: F,
        handler: HandlerLease<'scope, E>,
        errors: &'scope ErrorSlot<E>,
    ) -> Self
    where
        T: 'scope,
        E: 'scope,
        F: FnMut(Option<&T>) -> Result<T, E> + 'scope,
    {
        Self {
            slot,
            callback: Box::new(callback),
            pending: None,
            handler,
            errors,
        }
    }
}

impl<'scope, T, E> ComputationBehavior<'scope> for MemoBehavior<'scope, T, E>
where
    T: PartialEq + 'scope,
    E: 'scope,
{
    fn execute(
        &mut self,
        scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>> {
        let old = self
            .slot
            .try_read(scheduler.clone())
            .map_err(ComputationExecutionError::Runtime)?;
        let new = (self.callback)(old.as_ref()).map_err(|error| {
            ComputationExecutionError::Callback(ErrorEvent::new(error, &self.handler, self.errors))
        })?;
        let changed = old.as_ref().is_none_or(|value| *value != new);
        drop(old);
        if changed {
            self.pending = Some(new);
        }
        Ok(ComputationOutcome {
            commit_value: changed,
            notify: changed,
            stop_after_run: false,
        })
    }

    fn commit(&mut self, scheduler: std::rc::Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<()> {
        let mut value = self.slot.try_write(scheduler)?;
        *value = self.pending.take();
        Ok(())
    }

    fn discard_pending(&mut self) {
        self.pending = None;
    }

    fn has_value(&self) -> bool {
        self.slot.is_initialized()
    }

    fn clear(&mut self) {
        self.pending = None;
        self.slot.clear();
    }
}

pub(crate) type DerivedCallback<'scope, T, E> = Box<dyn FnMut() -> Result<T, E> + 'scope>;

pub(crate) struct DerivedBehavior<'scope, T, E> {
    slot: &'scope TypedSlot<T>,
    callback: DerivedCallback<'scope, T, E>,
    pending: Option<T>,
    handler: HandlerLease<'scope, E>,
    errors: &'scope ErrorSlot<E>,
}

impl<'scope, T, E> DerivedBehavior<'scope, T, E> {
    pub(crate) fn new<F>(
        slot: &'scope TypedSlot<T>,
        callback: F,
        handler: HandlerLease<'scope, E>,
        errors: &'scope ErrorSlot<E>,
    ) -> Self
    where
        T: 'scope,
        E: 'scope,
        F: FnMut() -> Result<T, E> + 'scope,
    {
        Self {
            slot,
            callback: Box::new(callback),
            pending: None,
            handler,
            errors,
        }
    }
}

impl<'scope, T, E> ComputationBehavior<'scope> for DerivedBehavior<'scope, T, E>
where
    T: 'scope,
    E: 'scope,
{
    fn execute(
        &mut self,
        _scheduler: std::rc::Rc<RefCell<GlobalScheduler>>,
    ) -> Result<ComputationOutcome, ComputationExecutionError<'scope>> {
        self.pending = Some((self.callback)().map_err(|error| {
            ComputationExecutionError::Callback(ErrorEvent::new(error, &self.handler, self.errors))
        })?);
        Ok(ComputationOutcome {
            commit_value: true,
            notify: true,
            stop_after_run: false,
        })
    }

    fn commit(&mut self, scheduler: std::rc::Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<()> {
        let mut value = self.slot.try_write(scheduler)?;
        *value = self.pending.take();
        Ok(())
    }

    fn discard_pending(&mut self) {
        self.pending = None;
    }

    fn has_value(&self) -> bool {
        self.slot.is_initialized()
    }

    fn clear(&mut self) {
        self.pending = None;
        self.slot.clear();
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
