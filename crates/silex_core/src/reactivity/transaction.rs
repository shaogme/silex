//! High-level staged transaction API with `SilexError` conversion.

use crate::{SilexError, SilexErrorKind, SilexResult, reactivity::Signal};
use silex_reactivity::{
    ReactiveTransaction, Signal as RawSignal, TransactionError, TransactionOperationError,
    TransactionPhase,
};

/// A staged multi-signal transaction bound to one owner scope.
pub struct Transaction<'scope> {
    inner: ReactiveTransaction<'scope>,
}

impl<'scope> Transaction<'scope> {
    pub(crate) fn from_inner(inner: ReactiveTransaction<'scope>) -> Self {
        Self { inner }
    }

    /// Return the current transaction lifecycle phase.
    pub fn phase(&self) -> TransactionPhase {
        self.inner.phase()
    }

    /// Read an untracked clone of a signal's current value.
    pub fn snapshot<T>(&self, source: Signal<'scope, T>) -> SilexResult<T>
    where
        T: Clone + 'scope,
    {
        self.inner
            .snapshot(raw_signal(source)?)
            .map_err(map_transaction_error)
    }

    /// Stage one typed update and return the user's operation result.
    pub fn update<T, R, F>(&mut self, target: Signal<'scope, T>, f: F) -> SilexResult<R>
    where
        T: Clone + 'scope,
        F: FnOnce(&mut T) -> SilexResult<R>,
    {
        self.inner
            .update(raw_signal(target)?, f)
            .map_err(map_operation_error)
    }

    /// Stage a replacement value for one signal.
    pub fn set<T>(&mut self, target: Signal<'scope, T>, value: T) -> SilexResult<()>
    where
        T: Clone + 'scope,
    {
        self.inner
            .set(raw_signal(target)?, value)
            .map_err(map_transaction_error)
    }

    /// Publish all staged values as one runtime transaction.
    pub fn commit(self) -> SilexResult<()> {
        self.inner.commit().map_err(map_transaction_error)
    }

    /// Discard all staged values.
    pub fn abort(self) -> SilexResult<()> {
        self.inner.abort().map_err(map_transaction_error)
    }
}

fn map_transaction_error(error: TransactionError) -> SilexError {
    SilexError::fatal(SilexErrorKind::Transaction(Box::new(error)))
}

fn raw_signal<'scope, T>(signal: Signal<'scope, T>) -> SilexResult<RawSignal<'scope, T>>
where
    T: 'scope,
{
    RawSignal::from_pair((signal.read.inner, signal.write.inner)).map_err(SilexError::fatal)
}

fn map_operation_error(error: TransactionOperationError<SilexError>) -> SilexError {
    match error {
        TransactionOperationError::Runtime(error) => map_transaction_error(error),
        TransactionOperationError::User(error) => error,
    }
}
