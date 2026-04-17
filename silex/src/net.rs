mod backend;
mod builder;
mod codec;
mod state;

pub use backend::{
    BrowserTransport, EventStream, EventStreamBuilder, EventStreamConnection, HttpBackend,
    Transport, WebSocket, WebSocketBuilder, WebSocketConnection,
};
pub use builder::{HttpClient, HttpClientBuilder, IntoNetValue};
#[cfg(feature = "json")]
pub use codec::NetJsonCodec;
pub use codec::{ResponseCodec, TextCodec};
pub use state::{
    CachePolicy, ConnectionState, EventMessage, HttpMethod, HttpResponse, RequestBody, RequestSpec,
    RetryPolicy,
};

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

