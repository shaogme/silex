use std::{fmt, marker::PhantomData};

use crate::{SilexError, SilexResult};

/// A typed callback owned by a scope.
pub struct Callback<'scope, T = ()> {
    pub(crate) inner: silex_reactivity::Callback<'scope, T>,
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
    pub(crate) fn from_inner(inner: silex_reactivity::Callback<'scope, T>) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }

    /// Invoke the callback and preserve the underlying reactive error.
    pub fn invoke(&self, value: T) -> SilexResult<()> {
        self.inner
            .invoke(value)
            .map_err(|error| SilexError::Reactivity(error.to_string()))
    }

    /// Invoke the callback using the legacy method spelling.
    ///
    /// The return type is intentionally the same as [`Self::invoke`]; stale
    /// callbacks are errors rather than a lossy boolean status.
    pub fn call(&self, value: T) -> SilexResult<()> {
        self.invoke(value)
    }
}
