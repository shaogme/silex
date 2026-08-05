use silex_core::{Runtime, SilexError, SilexResult};
use silex_router::{RouterContext, RouterContextProps};

fn assert_runtime_mismatch<'scope>(result: SilexResult<RouterContext<'scope>>) {
    assert!(matches!(
        result,
        Err(SilexError::Reactivity(message)) if message.contains("不同")
    ));
}

#[test]
fn foreign_search_is_rejected_before_query_memo_creation() {
    let mut source_runtime = Runtime::new();
    let source_root = source_runtime.run();
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run();

    let foreign_search = source_root.with_scope(|scope| {
        let (search, _) = scope.signal(String::from("?foreign=true"));
        search
    });

    target_root.with_scope(|scope| {
        let (path, set_path) = scope.signal(String::from("/"));
        let (_, set_search) = scope.signal(String::new());
        let result = RouterContext::try_new(
            scope,
            RouterContextProps {
                base_path: String::from("/"),
                path,
                search: foreign_search,
                set_path,
                set_search,
            },
        );

        assert_runtime_mismatch(result);
    });
}

#[test]
fn foreign_write_destination_is_rejected_before_context_creation() {
    let mut source_runtime = Runtime::new();
    let source_root = source_runtime.run();
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime.run();

    let foreign_set_path = source_root.with_scope(|scope| {
        let (_, set_path) = scope.signal(String::from("/foreign"));
        set_path
    });

    target_root.with_scope(|scope| {
        let (path, _) = scope.signal(String::from("/"));
        let (search, set_search) = scope.signal(String::new());
        let result = RouterContext::try_new(
            scope,
            RouterContextProps {
                base_path: String::from("/"),
                path,
                search,
                set_path: foreign_set_path,
                set_search,
            },
        );

        assert_runtime_mismatch(result);
    });
}
