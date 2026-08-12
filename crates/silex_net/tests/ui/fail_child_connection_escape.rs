use silex_core::Runtime;
use silex_net::{WebSocket, WebSocketConnection};

fn escape(runtime: &mut Runtime) -> WebSocketConnection<'static> {
    runtime.child(|scope| {
        WebSocket::lazy(
            scope,
            "wss://example.test",
            scope.error_handler(|_| {}).unwrap(),
        )
            .build()
            .unwrap()
    })
    .unwrap()
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = escape(&mut runtime);
}
