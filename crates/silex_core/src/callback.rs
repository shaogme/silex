use silex_reactivity::AnyValue;
use std::{fmt, marker::PhantomData};

/// A typed callback owned by a scope.
pub struct Callback<'scope, T = ()> {
    pub(crate) inner: silex_reactivity::Callback<'scope>,
    marker: PhantomData<fn(T)>,
}

impl<'scope, T> Copy for Callback<'scope, T> {}

impl<'scope, T> Clone for Callback<'scope, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> fmt::Debug for Callback<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Callback").finish_non_exhaustive()
    }
}

impl<'scope, T: 'scope> Callback<'scope, T> {
    pub(crate) fn from_inner(inner: silex_reactivity::Callback<'scope>) -> Self {
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
