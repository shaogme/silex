use silex_reactivity::AnyValue;
use std::{fmt, marker::PhantomData};

/// A typed callback owned by a scope.
pub struct Callback<'scope, 'run, T = ()> {
    pub(crate) inner: silex_reactivity::Callback<'scope, 'run>,
    marker: PhantomData<fn(T)>,
}

impl<'scope, 'run, T> Copy for Callback<'scope, 'run, T> {}

impl<'scope, 'run, T> Clone for Callback<'scope, 'run, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for Callback<'_, '_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Callback").finish_non_exhaustive()
    }
}

impl<'scope, 'run, T: 'scope> Callback<'scope, 'run, T> {
    pub(crate) fn from_inner(inner: silex_reactivity::Callback<'scope, 'run>) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }

    pub fn call(&self, value: T) -> bool {
        self.inner.invoke(AnyValue::new(value)).is_ok()
    }

    pub fn is_alive(&self) -> bool {
        self.inner.is_alive()
    }
}
