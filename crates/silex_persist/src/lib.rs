mod backend;
mod builder;
mod codec;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistMode {
    Immediate,
    Manual,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStrategy {
    None,
    CrossContext,
    Debounce(std::time::Duration),
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
