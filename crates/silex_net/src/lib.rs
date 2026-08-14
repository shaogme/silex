mod backend;
mod builder;
mod codec;
mod state;

pub use backend::{
    BrowserTransport, EventStream, EventStreamBuilder, EventStreamConnection, HttpBackend,
    Transport, TransportFuture, WebSocket, WebSocketBuilder, WebSocketConnection,
};
#[cfg(feature = "persist")]
pub use builder::HttpCache;
pub use builder::{HttpClient, HttpClientBuilder, IntoNetValue, ValueResolver};
#[cfg(feature = "persist")]
pub use codec::CacheCodec;
#[cfg(feature = "json")]
pub use codec::NetJsonCodec;
pub use codec::{ResponseCodec, TextCodec};
pub use silex_core::{NetConnectionState as ConnectionState, NetError, NetErrorKind};
#[cfg(feature = "persist")]
pub use state::{CacheConfig, CacheEviction};
pub use state::{
    CachePolicy, CredentialsMode, EventMessage, HttpMethod, HttpResponse, RequestBody, RequestSpec,
    RetryPolicy,
};

#[cfg(feature = "persist")]
pub mod persist {
    pub use silex_persist::*;
}

pub mod reexports {
    pub use gloo_timers;
    pub use js_sys;
    #[cfg(feature = "json")]
    pub use serde_json;
    pub use wasm_bindgen;
    pub use wasm_bindgen_futures;
    pub use web_sys;
}

#[cfg(test)]
mod tests {
    use super::{NetError, NetErrorKind};

    #[test]
    fn abort_is_not_retryable_but_timeout_is() {
        assert!(!NetError::recoverable(NetErrorKind::Aborted).is_retryable());
        assert!(NetError::recoverable(NetErrorKind::Timeout).is_retryable());
        assert!(NetError::is_retryable_http_status(503));
        assert!(!NetError::is_retryable_http_status(404));
    }
}
