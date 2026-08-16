use silex_core::{ErrorHandlerToken, Runtime};
use silex_net::{EventStream, NetError, NetErrorKind, WebSocket};

fn test_handler<'scope>(scope: silex_core::OwnerAccess<'scope>) -> ErrorHandlerToken<'scope> {
    scope.error_handler(|_| {}).unwrap()
}

#[test]
fn foreign_connection_url_is_rejected_before_host_registration() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.owner().expect("source runtime setup");
    let target_root = target_runtime.owner().expect("target runtime setup");

    let source_scope = source_root.access();
    let target_scope = target_root.access();
    let (url, _) = source_scope
        .signal("wss://foreign.test".to_string())
        .unwrap();
    let socket = WebSocket::lazy(target_scope, url, test_handler(target_scope)).build();
    let stream = EventStream::lazy(target_scope, url, test_handler(target_scope)).build();
    assert!(matches!(
        socket,
        Err(NetError::Fatal(NetErrorKind::Core(_)))
    ));
    assert!(matches!(
        stream,
        Err(NetError::Fatal(NetErrorKind::Core(_)))
    ));

    source_root.close().expect("source root cleanup");
    target_root.close().expect("target root cleanup");
}
