use silex_core::Runtime;
use silex_net::HttpClient;

#[test]
fn foreign_request_source_is_rejected_before_resource_creation() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.run();
    let target_root = target_runtime.run();
    source_root.with_scope(|source_scope| {
        let (source, _) = source_scope.signal(1_i32);
        target_root.with_scope(|target_scope| {
            let builder = HttpClient::get(target_scope, "https://example.test");
            let result = builder.try_as_resource(source, None);
            assert!(result.is_err());
        });
    });
    source_root.dispose().expect("source root cleanup");
    target_root.dispose().expect("target root cleanup");
}
