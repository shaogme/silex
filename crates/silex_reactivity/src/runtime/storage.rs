//! Stable node storage and short-lived dynamic leases.

use super::scheduler::GlobalScheduler;
use crate::{
    ReactiveError, ReactiveResult,
    internal::value::{AnyValue, CallbackThunk, Computation},
};
use std::{
    cell::{Ref, RefCell, RefMut},
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
        scheduler: Rc<RefCell<GlobalScheduler>>,
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
        scheduler: Rc<RefCell<GlobalScheduler>>,
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
}

impl<T> LeaseCell<Option<T>> {
    pub(crate) fn is_initialized(&self) -> bool {
        self.value.try_borrow().is_ok_and(|value| value.is_some())
    }
}

struct LeaseTicket {
    scheduler: Rc<RefCell<GlobalScheduler>>,
}

impl LeaseTicket {
    fn new(scheduler: Rc<RefCell<GlobalScheduler>>) -> ReactiveResult<Self> {
        scheduler
            .try_borrow_mut()
            .map_err(|_| ReactiveError::BorrowConflict)?
            .active_leases += 1;
        Ok(Self { scheduler })
    }
}

impl Drop for LeaseTicket {
    fn drop(&mut self) {
        let mut scheduler = self
            .scheduler
            .try_borrow_mut()
            .expect("scheduler lease count must not be borrowed while a lease drops");
        scheduler.active_leases = scheduler.active_leases.saturating_sub(1);
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

pub(crate) struct ComputationStorage<'scope> {
    pub(crate) value: LeaseCell<Option<AnyValue<'scope>>>,
    pub(crate) computation: LeaseCell<Computation<'scope>>,
}

impl<'scope> ComputationStorage<'scope> {
    pub(crate) fn new(computation: Computation<'scope>) -> Self {
        Self {
            value: LeaseCell::new(None),
            computation: LeaseCell::new(computation),
        }
    }
}

pub(crate) enum NodeStorage<'scope> {
    Value(LeaseCell<AnyValue<'scope>>),
    Computation(ComputationStorage<'scope>),
    Callback(LeaseCell<CallbackThunk<'scope>>),
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
