use silex_core::{Runtime, RxGet};
use silex_net::{EventStream, WebSocket};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|scope| {
        let url = scope
            .rw_signal("wss://example.test/socket".to_string())
            .unwrap();
        let (opened, set_opened) = scope.signal(false).unwrap();
        let socket = WebSocket::lazy(
            scope,
            url,
            scope.error_handler(|_| {}).unwrap(),
        )
        .on_open(move || set_opened.set(true).unwrap())
        .build()
        .unwrap();
        let stream = EventStream::lazy(
            scope,
            "https://example.test/events",
            scope.error_handler(|_| {}).unwrap(),
        )
        .max_messages(16)
            .build()
            .unwrap();
        let _ = (
            socket.state().get().unwrap(),
            stream.raw_messages().get().unwrap(),
            opened.get().unwrap(),
        );
    })
    .unwrap();
}
