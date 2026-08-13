use std::fmt;

use silex_core::{ReactiveError, SilexError, SilexErrorKind};

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

#[derive(Debug, Clone, PartialEq)]
pub enum PersistenceError {
    BackendUnavailable,
    ReadFailed(String),
    WriteFailed(String),
    RemoveFailed(String),
    DecodeFailed { raw: String, message: String },
    EncodeFailed(String),
    InvalidConfiguration(String),
    Reactivity(ReactiveError),
}

impl PersistenceError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::BackendUnavailable => "backend unavailable".to_string(),
            Self::ReadFailed(message)
            | Self::WriteFailed(message)
            | Self::RemoveFailed(message)
            | Self::EncodeFailed(message)
            | Self::InvalidConfiguration(message) => message.clone(),
            Self::DecodeFailed { message, .. } => message.clone(),
            Self::Reactivity(error) => error.to_string(),
        }
    }
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for PersistenceError {}

impl From<ReactiveError> for PersistenceError {
    fn from(error: ReactiveError) -> Self {
        Self::Reactivity(error)
    }
}

impl From<SilexError> for PersistenceError {
    fn from(error: SilexError) -> Self {
        match error.into_kind() {
            SilexErrorKind::Reactivity(error) => Self::Reactivity(error),
            kind => Self::InvalidConfiguration(kind.to_string()),
        }
    }
}

impl From<PersistenceError> for SilexError {
    fn from(error: PersistenceError) -> Self {
        match error {
            PersistenceError::Reactivity(error) => {
                SilexError::fatal(SilexErrorKind::Reactivity(error))
            }
            other => SilexError::recoverable(SilexErrorKind::Framework(other.message())),
        }
    }
}

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

    #[test]
    fn test_persistence_error_to_silex_error() {
        let err = PersistenceError::BackendUnavailable;
        let silex_err: SilexError = err.into();
        assert!(matches!(
            silex_err,
            SilexError::Recoverable(SilexErrorKind::Framework(msg))
                if msg == "backend unavailable"
        ));

        let err = PersistenceError::ReadFailed("read error".to_string());
        let silex_err: SilexError = err.into();
        assert!(matches!(
            silex_err,
            SilexError::Recoverable(SilexErrorKind::Framework(msg)) if msg == "read error"
        ));

        let reactive_err = ReactiveError::NoSuchNode;
        let err = PersistenceError::Reactivity(reactive_err);
        let silex_err: SilexError = err.into();
        assert!(matches!(
            silex_err,
            SilexError::Fatal(SilexErrorKind::Reactivity(ReactiveError::NoSuchNode))
        ));
    }
}
