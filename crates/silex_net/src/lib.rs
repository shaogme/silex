mod backend;
mod builder;
mod codec;
mod state;

pub use backend::{
    BrowserTransport, EventStream, EventStreamBuilder, EventStreamConnection, HttpBackend,
    Transport, TransportFuture, WebSocket, WebSocketBuilder, WebSocketConnection,
};
pub use builder::{HttpClient, HttpClientBuilder, IntoNetValue, ValueResolver};
#[cfg(feature = "json")]
pub use codec::NetJsonCodec;
pub use codec::{ResponseCodec, TextCodec};
pub use state::{
    CachePolicy, ConnectionState, EventMessage, HttpMethod, HttpResponse, RequestBody, RequestSpec,
    RetryPolicy,
};

pub mod reexports {
    pub use gloo_timers;
    pub use js_sys;
    #[cfg(feature = "json")]
    pub use serde_json;
    pub use wasm_bindgen;
    pub use wasm_bindgen_futures;
    pub use web_sys;
}

use wasm_bindgen::JsValue;

#[derive(Clone, Debug, PartialEq)]
pub enum NetError {
    BrowserUnavailable,
    TransportUnavailable,
    Timeout,
    Aborted,
    HttpStatus { status: u16, body: String },
    DecodeError(String),
    SerializeError(String),
    ConnectionClosed(String),
    JsError(String),
    InvalidConfiguration(String),
}

impl From<JsValue> for NetError {
    fn from(value: JsValue) -> Self {
        Self::JsError(format!("{value:?}"))
    }
}

impl NetError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Timeout | Self::TransportUnavailable => true,
            Self::HttpStatus { status, .. } => Self::is_retryable_http_status(*status),
            _ => false,
        }
    }

    pub fn is_retryable_http_status(status: u16) -> bool {
        matches!(status, 408 | 429 | 500..=599)
    }
}

#[cfg(test)]
mod tests {
    use super::NetError;

    #[test]
    fn abort_is_not_retryable_but_timeout_is() {
        assert!(!NetError::Aborted.is_retryable());
        assert!(NetError::Timeout.is_retryable());
        assert!(NetError::is_retryable_http_status(503));
        assert!(!NetError::is_retryable_http_status(404));
    }
}
