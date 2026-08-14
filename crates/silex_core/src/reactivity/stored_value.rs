use crate::{Rx, RxValueKind, Scope, SilexError, SilexResult};
use std::fmt;

/// A non-reactive value owned by a scope.
///
/// During final disposal of the owning scope, `with` and `update` remain
/// available until this value's payload is dropped. The
/// owner is still inactive in that window, so raw signals, callbacks, node
/// refs, node creation, and other scope APIs remain unavailable. A
/// `Signal` facade created from this value preserves this StoredValue source
/// kind and therefore follows the same exception. This exception only applies
/// to final scope disposal; it does not apply to effect reruns or single-node
/// stops, and a handle must not be used asynchronously after the cleanup
/// callback returns.
pub struct StoredValue<'scope, T> {
    pub(crate) inner: silex_reactivity::StoredValue<'scope, T>,
    pub(crate) scope: Scope<'scope>,
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
        self.inner == other.inner && self.scope == other.scope
    }
}

impl<'scope, T> Eq for StoredValue<'scope, T> {}

impl<'scope, T: 'scope> StoredValue<'scope, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::StoredValue<'scope, T>,
        scope: Scope<'scope>,
    ) -> Self {
        Self { inner, scope }
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> SilexResult<U> {
        self.inner.with(f).map_err(SilexError::fatal)
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> SilexResult<U> {
        self.inner.update(f).map_err(SilexError::fatal)
    }

    pub fn set(&self, value: T) -> SilexResult<()> {
        self.update(|stored| *stored = value)
    }

    pub fn into_rx(self) -> Rx<'scope, T, RxValueKind> {
        Rx::from_stored(self)
    }
}
