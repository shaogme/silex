use crate::{
    OwnerAccess, Rx, SilexError, SilexResult,
    traits::{RuntimeScoped, RxBase, RxRead, RxValue, RxWrite},
};
use std::fmt;

/// A non-reactive value owned by a scope.
///
/// During final disposal of the owning scope, `with` and `update` remain
/// available until this value's payload is dropped. The
/// owner is still inactive in that window, so raw signals, callbacks, node
/// refs, node creation, and other scope APIs remain unavailable. A
/// `Rx` created from this value preserves this StoredValue source
/// kind and therefore follows the same exception. This exception only applies
/// to final scope disposal; it does not apply to effect reruns or single-node
/// stops, and a handle must not be used asynchronously after the cleanup
/// callback returns.
pub struct StoredValue<'scope, T> {
    pub(crate) inner: silex_reactivity::StoredValue<'scope, T>,
    pub(crate) owner: OwnerAccess<'scope>,
}

impl<'scope, T> Copy for StoredValue<'scope, T> {}

impl<'scope, T> Clone for StoredValue<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for StoredValue<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredValue").finish_non_exhaustive()
    }
}

impl<'scope, T> PartialEq for StoredValue<'scope, T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner && self.owner == other.owner
    }
}

impl<'scope, T> Eq for StoredValue<'scope, T> {}

impl<'scope, T: 'scope> StoredValue<'scope, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::StoredValue<'scope, T>,
        owner: OwnerAccess<'scope>,
    ) -> Self {
        Self { inner, owner }
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    pub fn set(&self, value: T) -> SilexResult<()> {
        self.update(|stored| *stored = value)
    }

    pub fn into_rx(self) -> Rx<'scope, T> {
        Rx::from_stored(self)
    }
}

impl<'scope, T> RuntimeScoped for StoredValue<'scope, T> {
    fn owner_access(&self) -> OwnerAccess<'_> {
        self.owner
    }
}

impl<'scope, T> RxValue for StoredValue<'scope, T> {
    type Value = T;
}

impl<'scope, T> RxBase for StoredValue<'scope, T> {
    fn track(&self) -> SilexResult<()> {
        self.inner.track().map_err(SilexError::fatal)
    }
}

impl<'scope, T> RxRead for StoredValue<'scope, T> {
    fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }

    fn with_untracked<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with_untracked(f).map_err(SilexError::fatal)
    }
}

impl<'scope, T> RxWrite for StoredValue<'scope, T> {
    fn rx_update_untracked<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    fn rx_notify(&self) -> SilexResult<()> {
        Ok(())
    }
}
