//! Single dynamic-borrow boundary for the single-threaded runtime.

use std::{
    cell::{Ref, RefCell, RefMut},
    ops::{Deref, DerefMut},
    rc::Rc,
};

/// Internal storage location used to classify borrow diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BorrowSite {
    ScopeState,
    Scheduler,
    Payload,
    Handler,
    OwnerRegistry,
    CloseReport,
    ObserverStack,
}

/// A failed dynamic borrow together with its internal storage location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BorrowFailure {
    site: BorrowSite,
}

impl BorrowFailure {
    #[inline]
    pub(crate) const fn site(self) -> BorrowSite {
        self.site
    }
}

/// A `RefCell` whose only exposed operations are fallible borrows.
pub(crate) struct BorrowCell<T> {
    value: RefCell<T>,
    site: BorrowSite,
}

impl<T> BorrowCell<T> {
    #[inline]
    pub(crate) const fn new(value: T, site: BorrowSite) -> Self {
        Self {
            value: RefCell::new(value),
            site,
        }
    }

    #[inline]
    pub(crate) fn try_read(&self) -> Result<BorrowRef<'_, T>, BorrowFailure> {
        self.value
            .try_borrow()
            .map(|value| BorrowRef { value })
            .map_err(|_| BorrowFailure { site: self.site })
    }

    #[inline]
    pub(crate) fn try_write(&self) -> Result<BorrowRefMut<'_, T>, BorrowFailure> {
        self.value
            .try_borrow_mut()
            .map(|value| BorrowRefMut { value })
            .map_err(|_| BorrowFailure { site: self.site })
    }

    #[inline]
    pub(crate) fn try_borrow(&self) -> Result<BorrowRef<'_, T>, BorrowFailure> {
        self.try_read()
    }

    #[inline]
    pub(crate) fn try_borrow_mut(&self) -> Result<BorrowRefMut<'_, T>, BorrowFailure> {
        self.try_write()
    }
}

/// Shared scope-local storage with a fallible dynamic-borrow boundary.
pub(crate) type SharedCell<T> = Rc<BorrowCell<T>>;

/// Read guard returned by [`BorrowCell::try_read`].
pub(crate) struct BorrowRef<'a, T: ?Sized> {
    value: Ref<'a, T>,
}

impl<T: ?Sized> Deref for BorrowRef<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<'a, T: ?Sized> BorrowRef<'a, T> {
    pub(crate) fn try_map<U: ?Sized>(
        self,
        f: impl FnOnce(&T) -> Option<&U>,
    ) -> Result<BorrowRef<'a, U>, Self> {
        match Ref::filter_map(self.value, f) {
            Ok(value) => Ok(BorrowRef { value }),
            Err(value) => Err(BorrowRef { value }),
        }
    }
}

/// Write guard returned by [`BorrowCell::try_write`].
pub(crate) struct BorrowRefMut<'a, T: ?Sized> {
    value: RefMut<'a, T>,
}

impl<T: ?Sized> Deref for BorrowRefMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: ?Sized> DerefMut for BorrowRefMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<'a, T: ?Sized> BorrowRefMut<'a, T> {
    pub(crate) fn try_map<U: ?Sized>(
        self,
        f: impl FnOnce(&mut T) -> Option<&mut U>,
    ) -> Result<BorrowRefMut<'a, U>, Self> {
        match RefMut::filter_map(self.value, f) {
            Ok(value) => Ok(BorrowRefMut { value }),
            Err(value) => Err(BorrowRefMut { value }),
        }
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

    #[test]
    fn conflicting_borrows_are_reported_with_their_site() {
        let cell = BorrowCell::new(1_u32, BorrowSite::ScopeState);
        let read = cell.try_read().expect("first read should succeed");
        assert!(matches!(
            cell.try_write(),
            Err(BorrowFailure {
                site: BorrowSite::ScopeState
            })
        ));
        drop(read);
        *cell.try_write().expect("write after read should succeed") = 2;
        assert_eq!(
            *cell.try_read().expect("read after write should succeed"),
            2
        );
    }
}
