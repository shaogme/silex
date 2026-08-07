use std::{panic::AssertUnwindSafe, panic::catch_unwind};

use silex_core::{ErrorReporter, Runtime, runtime_inputs_of};
use silex_net::{HttpClient, NetError, ValueResolver};

fn test_handler<'scope>() -> ErrorReporter<'scope> {
    ErrorReporter::new(|_| {})
}

#[test]
fn foreign_request_source_is_rejected_before_resource_creation() {
    let mut source_runtime = Runtime::new();
    let mut target_runtime = Runtime::new();
    let source_root = source_runtime.run();
    let target_root = target_runtime.run();
    source_root.with_scope(|source_scope| {
        let (source, _) = source_scope.signal(1_i32);
        target_root.with_scope(|target_scope| {
            let builder = HttpClient::get(target_scope, "https://example.test", test_handler());
            let result = builder.try_as_resource(source, None);
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
    let source_root = source_runtime.run();
    let target_root = target_runtime.run();

    source_root.with_scope(|source_scope| {
        let (source, _) = source_scope.signal(1_i32);
        let foreign_inputs = runtime_inputs_of(source);
        target_root.with_scope(|target_scope| {
            let foreign_url = ValueResolver::dynamic_with_inputs(
                || "https://example.test".to_string(),
                || "https://example.test".to_string(),
                foreign_inputs.clone(),
            );
            let before = target_scope.runtime_snapshot();
            let result =
                HttpClient::get(target_scope, foreign_url, test_handler()).try_into_resource(None);

            assert!(matches!(result, Err(NetError::InvalidConfiguration(_))));
            assert_eq!(target_scope.runtime_snapshot(), before);

            let foreign_url = ValueResolver::dynamic_with_inputs(
                || "https://example.test".to_string(),
                || "https://example.test".to_string(),
                foreign_inputs,
            );
            let before_panic = target_scope.runtime_snapshot();
            let result = catch_unwind(AssertUnwindSafe(|| {
                HttpClient::get(target_scope, foreign_url, test_handler()).into_resource(None)
            }));

            assert!(result.is_err());
            assert_eq!(target_scope.runtime_snapshot(), before_panic);
        });
    });

    source_root.dispose().expect("source root cleanup");
    target_root.dispose().expect("target root cleanup");
}
