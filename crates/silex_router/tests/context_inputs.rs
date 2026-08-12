use silex_core::{ErrorReporter, ReactiveError, Runtime, Scope, SilexError, SilexResult};
use silex_router::{RouterContext, RouterContextProps};

fn test_handler<'scope>(scope: Scope<'scope>) -> ErrorReporter<'scope> {
    scope
        .error_handler(|_| {})
        .expect("test error handler should be registered")
}

fn assert_runtime_mismatch<'scope>(result: SilexResult<RouterContext<'scope>>) {
    assert!(matches!(
        result,
        Err(SilexError::Reactivity(ReactiveError::RuntimeMismatch))
    ));
}

#[test]
fn foreign_search_is_rejected_before_query_memo_creation() {
    let mut source_runtime = Runtime::new();
    let source_root = source_runtime.run().expect("source root should be created");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run().expect("target root should be created");

    let foreign_search = source_root.with_scope(|scope| {
        let (search, _) = scope
            .signal(String::from("?foreign=true"))
            .expect("foreign search signal should be created");
        search
    });

    target_root.with_scope(|scope| {
        let (path, set_path) = scope
            .signal(String::from("/"))
            .expect("path signal should be created");
        let (_, set_search) = scope
            .signal(String::new())
            .expect("search signal should be created");
        let result = RouterContext::new(
            scope,
            RouterContextProps {
                base_path: String::from("/"),
                path,
                search: foreign_search,
                set_path,
                set_search,
            },
            test_handler(scope),
        );

        assert_runtime_mismatch(result);
    });
}

#[test]
fn foreign_write_destination_is_rejected_before_context_creation() {
    let mut source_runtime = Runtime::new();
    let source_root = source_runtime.run().expect("source root should be created");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run().expect("target root should be created");

    let foreign_set_path = source_root.with_scope(|scope| {
        let (_, set_path) = scope
            .signal(String::from("/foreign"))
            .expect("foreign path signal should be created");
        set_path
    });

    target_root.with_scope(|scope| {
        let (path, _) = scope
            .signal(String::from("/"))
            .expect("path signal should be created");
        let (search, set_search) = scope
            .signal(String::new())
            .expect("search signal should be created");
        let result = RouterContext::new(
            scope,
            RouterContextProps {
                base_path: String::from("/"),
                path,
                search,
                set_path: foreign_set_path,
                set_search,
            },
            test_handler(scope),
        );

        assert_runtime_mismatch(result);
    });
}
