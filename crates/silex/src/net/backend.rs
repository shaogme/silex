use std::{future::Future, pin::Pin};

use crate::net::state::HttpResponse;

pub mod event_stream;
pub mod fetch;
pub mod websocket;

pub use event_stream::{EventStream, EventStreamBuilder, EventStreamConnection};
pub use fetch::{BrowserTransport, HttpBackend, Transport};
pub use websocket::{WebSocket, WebSocketBuilder, WebSocketConnection};

pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<HttpResponse, crate::net::NetError>> + 'a>>;
