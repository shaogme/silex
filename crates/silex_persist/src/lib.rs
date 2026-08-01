mod backend;
mod builder;
mod codec;
mod state;

pub use backend::{
    BackendEvent, BackendSubscription, LocalStorageBackend, PersistenceBackend, QueryBackend,
    SessionStorageBackend, WebStorageBackend,
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
