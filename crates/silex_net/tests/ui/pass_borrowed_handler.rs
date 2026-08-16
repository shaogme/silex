use silex_core::Runtime;
use silex_net::{EventStream, HttpClient, WebSocket};

fn main() {
    let mut runtime = Runtime::new();
    if false {
        runtime
            .with_transient(|scope| {
                let token = scope.error_handler(|_| {}).unwrap();
                let _resource = HttpClient::get(scope, "https://example.test", &token)
                    .into_resource(None)
                    .unwrap();
                let _socket = WebSocket::lazy(scope, "wss://example.test", &token)
                    .build()
                    .unwrap();
                let _stream = EventStream::lazy(scope, "https://example.test/events", &token)
                    .build()
                    .unwrap();
            })
            .unwrap();
    }
}
