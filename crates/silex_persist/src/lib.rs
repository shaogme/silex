mod backend;
mod builder;
mod codec;
mod runtime;
mod state;

pub use backend::{
    BackendEvent, BackendEventSink, BackendSubscribeError, BackendSubscription,
    LocalStorageBackend, PersistenceBackend, QueryBackend, SessionStorageBackend,
    WebStorageBackend,
};
pub use builder::{HasDefault, NoBackend, NoCodec, NoDefault, PersistentBuilder};
#[cfg(feature = "json")]
pub use codec::PersistJsonCodec;
pub use codec::{OptionCodec, ParseCodec, PersistCodec, StringCodec};
pub use state::{DecodeErrorInfo, PersistenceState, Persistent};

pub use silex_core::{PersistenceError, PersistenceErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteDefault {
    Never,
    IfMissing,
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodePolicy {
    UseDefault,
    RemoveAndUseDefault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovePolicy {
    UseDefault,
    Ignore,
}

/// Controls when local mutations are written to the backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistWriteMode {
    /// Write each local mutation synchronously during the reactive update.
    Immediate,
    /// Keep local mutations in memory until [`Persistent::flush`] is called.
    Manual,
    /// Write only the latest local mutation after the debounce duration.
    ///
    /// The initial bootstrap write is never debounced. A failed scheduled
    /// write remains retryable through [`Persistent::flush`].
    Debounced(std::time::Duration),
}

/// Controls which backend-originated changes update this binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistExternalSync {
    /// Do not subscribe to backend change notifications.
    Disabled,
    /// Subscribe to Web Storage events and apply the latest external snapshot.
    StorageEvents,
    /// Subscribe to router query changes and apply the latest external
    /// snapshot.
    QueryChanges,
}

#[cfg(test)]
mod tests {
    use super::*;
    use silex_core::{ReactiveError, SilexError, SilexErrorKind};

    #[test]
    fn test_persistence_error_to_silex_error() {
        let err = PersistenceError::recoverable(PersistenceErrorKind::BackendUnavailable);
        let silex_err: SilexError = err.into();
        assert!(matches!(
            silex_err,
            SilexError::Recoverable(SilexErrorKind::Persistence(PersistenceError::Recoverable(
                PersistenceErrorKind::BackendUnavailable
            )))
        ));

        let err = PersistenceError::recoverable(PersistenceErrorKind::ReadFailed(
            "read error".to_string(),
        ));
        let silex_err: SilexError = err.into();
        assert!(matches!(
            silex_err,
            SilexError::Recoverable(SilexErrorKind::Persistence(
                PersistenceError::Recoverable(PersistenceErrorKind::ReadFailed(msg))
            )) if msg == "read error"
        ));

        let reactive_err = ReactiveError::NoSuchNode;
        let err = PersistenceError::fatal(PersistenceErrorKind::Reactivity(reactive_err));
        let silex_err: SilexError = err.into();
        assert!(matches!(
            silex_err,
            SilexError::Fatal(SilexErrorKind::Persistence(PersistenceError::Fatal(
                PersistenceErrorKind::Reactivity(ReactiveError::NoSuchNode)
            )))
        ));
    }
}
