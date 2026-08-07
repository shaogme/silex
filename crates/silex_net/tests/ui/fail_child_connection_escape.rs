use silex_core::Runtime;
use silex_net::{WebSocket, WebSocketConnection};

fn escape(runtime: &mut Runtime) -> WebSocketConnection<'static> {
    runtime.child(|scope| {
        WebSocket::lazy(scope, "wss://example.test", silex_core::ErrorReporter::new(|_| {}))
            .build()
    })
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = escape(&mut runtime);
}
