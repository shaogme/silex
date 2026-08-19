use std::{fmt, marker::PhantomData};

use crate::{SilexError, SilexErrorKind, SilexResult};
use silex_reactivity::{CallbackInvokeError, CompletionSubmitError, ReactiveError};

pub(crate) fn map_callback_error(error: CallbackInvokeError<SilexError>) -> SilexError {
    match error {
        CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
        CallbackInvokeError::User(error) => error,
        CallbackInvokeError::Handler(error) => SilexError::fatal(ReactiveError::Handler(error)),
    }
}

pub(crate) fn report_completion_error(
    error: CompletionSubmitError<SilexError>,
    mut report: impl FnMut(SilexError),
) {
    let (callback, close) = error.into_parts();
    if let Some(callback) = callback {
        report(map_callback_error(callback));
    }
    if let Some(close) = close {
        report(SilexError::fatal(SilexErrorKind::Close(close)));
    }
}

/// A typed callback owned by a scope.
pub struct Callback<'scope, T = ()> {
    pub(crate) inner: silex_reactivity::Callback<'scope, T, SilexError>,
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
    pub(crate) fn from_inner(inner: silex_reactivity::Callback<'scope, T, SilexError>) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }

    /// Invoke the callback and preserve the underlying reactive error.
    pub fn invoke(&self, value: T) -> SilexResult<()> {
        self.inner.invoke(value).map_err(|error| match error {
            CallbackInvokeError::Runtime(error) => SilexError::fatal(error),
            CallbackInvokeError::User(error) => error,
            CallbackInvokeError::Handler(error) => SilexError::fatal(ReactiveError::Handler(error)),
        })
    }

    /// Invoke the callback using the legacy method spelling.
    ///
    /// The return type is intentionally the same as [`Self::invoke`]; stale
    /// callbacks are errors rather than a lossy boolean status.
    pub fn call(&self, value: T) -> SilexResult<()> {
        self.invoke(value)
    }
}
