use silex_core::Runtime;
use silex_net::{EventStream, WebSocket};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let url = scope.rw_signal("wss://example.test/socket".to_string());
        let (opened, set_opened) = scope.signal(false);
        let socket = WebSocket::lazy(scope, url, scope.error_handler(|_| {}))
            .on_open(move || set_opened.set(true))
            .build();
        let stream = EventStream::lazy(
            scope,
            "https://example.test/events",
            scope.error_handler(|_| {}),
        )
            .max_messages(16)
            .build();
        let _ = (socket.state().get(), stream.raw_messages().get(), opened.get());
    });
}
