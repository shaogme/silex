use silex_core::{ErrorHandlerToken, Runtime};
use silex_net::{HttpClient, NetError, NetErrorKind};

fn test_handler<'scope>(scope: silex_core::Scope<'scope>) -> ErrorHandlerToken<'scope> {
    scope.error_handler(|_| {}).unwrap()
}

#[test]
fn foreign_request_source_is_rejected_before_resource_creation() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.run().expect("source runtime setup");
    let target_root = target_runtime.run().expect("target runtime setup");
    source_root.with_scope(|source_scope| {
        let (source, _) = source_scope.signal(1_i32).unwrap();
        target_root.with_scope(|target_scope| {
            let builder = HttpClient::get(
                target_scope,
                "https://example.test",
                test_handler(target_scope),
            );
            let result = builder.as_resource(source, None);
            assert!(result.is_err());
        });
    });
    source_root.dispose().expect("source root cleanup");
    target_root.dispose().expect("target root cleanup");
}

#[test]
fn foreign_builder_into_resource_is_transactional_before_target_creation() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.run().expect("source runtime setup");
    let target_root = target_runtime.run().expect("target runtime setup");

    source_root.with_scope(|source_scope| {
        let (source, _) = source_scope.signal(1_i32).unwrap();
        target_root.with_scope(|target_scope| {
            let handler = test_handler(target_scope);
            let before = target_scope.runtime_snapshot();
            let result = HttpClient::get(target_scope, source, &handler).into_resource(None);

            assert!(matches!(
                result,
                Err(NetError::Fatal(NetErrorKind::Core(_)))
            ));
            assert_eq!(target_scope.runtime_snapshot(), before);
        });
    });

    source_root.dispose().expect("source root cleanup");
    target_root.dispose().expect("target root cleanup");
}
