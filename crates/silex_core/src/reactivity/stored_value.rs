use crate::{Rx, RxValueKind, Scope};
use silex_reactivity::ReactiveResult;
use std::fmt;

/// A non-reactive value owned by a scope.
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

impl<'scope, T: 'scope> StoredValue<'scope, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::StoredValue<'scope, T>,
        scope: Scope<'scope>,
    ) -> Self {
        Self { inner, scope }
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.inner.with(f)
    }

    pub fn try_with<U>(&self, f: impl FnOnce(&T) -> U) -> ReactiveResult<U> {
        self.inner.try_with(f)
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        self.inner.update(f)
    }

    pub fn try_update<U>(&self, f: impl FnOnce(&mut T) -> U) -> ReactiveResult<U> {
        self.inner.try_update(f)
    }

    pub fn set(&self, value: T) {
        self.update(|stored| *stored = value);
    }

    pub fn into_rx(self) -> Rx<'scope, T, RxValueKind> {
        Rx::from_stored(self)
    }
}
