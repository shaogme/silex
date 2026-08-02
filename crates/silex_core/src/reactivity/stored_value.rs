use crate::{Rx, RxValueKind, Scope};
use std::fmt;

/// A non-reactive value owned by a scope.
pub struct StoredValue<'scope, 'run, T> {
    pub(crate) inner: silex_reactivity::StoredValue<'scope, 'run, T>,
    pub(crate) scope: Scope<'scope, 'run>,
}

impl<'scope, 'run, T> Copy for StoredValue<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for StoredValue<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for StoredValue<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredValue").finish_non_exhaustive()
    }
}

impl<'scope, 'run, T: 'scope> StoredValue<'scope, 'run, T> {
    pub(crate) fn from_inner(
        inner: silex_reactivity::StoredValue<'scope, 'run, T>,
        scope: Scope<'scope, 'run>,
    ) -> Self {
        Self { inner, scope }
    }

    pub fn with<U>(&self, f: impl FnOnce(&T) -> U) -> U {
        self.inner.with(f)
    }

    pub fn update<U>(&self, f: impl FnOnce(&mut T) -> U) -> U {
        self.inner.update(f)
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }

    pub fn set(&self, value: T) {
        self.update(|stored| *stored = value);
    }

    pub fn into_rx(self) -> Rx<'scope, 'run, T, RxValueKind> {
        Rx::from_stored(self)
    }
}
