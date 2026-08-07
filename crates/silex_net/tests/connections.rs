use silex_core::{ErrorReporter, Runtime};
use silex_net::{EventStream, NetError, WebSocket};

fn test_handler<'scope>() -> ErrorReporter<'scope> {
    ErrorReporter::new(|_| {})
}

#[test]
fn foreign_connection_url_is_rejected_before_host_registration() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.run();
    let target_root = target_runtime.run();

    source_root.with_scope(|source_scope| {
        let (url, _) = source_scope.signal("wss://foreign.test".to_string());
        target_root.with_scope(|target_scope| {
            let socket = WebSocket::lazy(target_scope, url, test_handler()).try_build();
            let stream = EventStream::lazy(target_scope, url, test_handler()).try_build();
            assert!(matches!(socket, Err(NetError::InvalidConfiguration(_))));
            assert!(matches!(stream, Err(NetError::InvalidConfiguration(_))));
        });
    });

    source_root.dispose().expect("source root cleanup");
    target_root.dispose().expect("target root cleanup");
}
