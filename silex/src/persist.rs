mod backend;
mod builder;
mod codec;
mod error;
mod policy;
mod state;

pub use backend::{
    BackendEvent, BackendSubscription, LocalStorageBackend, PersistenceBackend, QueryBackend,
    SessionStorageBackend,
};
pub use builder::{NoBackend, NoCodec, PersistentBuilder, persistent};
#[cfg(feature = "persistence")]
pub use codec::JsonCodec;
pub use codec::{OptionCodec, ParseCodec, PersistCodec, StringCodec};
pub use error::PersistenceError;
pub use policy::{DecodePolicy, PersistMode, RemovePolicy, SyncStrategy, WriteDefault};
pub use state::{DecodeErrorInfo, PersistenceState, Persistent};
