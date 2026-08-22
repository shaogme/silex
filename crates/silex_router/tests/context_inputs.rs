use silex_core::{
    ErrorHandlerToken, OwnerAccess, OwnerHandle, ReactiveError, Runtime, SilexContext, SilexError,
    SilexErrorKind, SilexResult,
};
use silex_router::{RouterContext, RouterContextProps};

fn test_handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("test error handler should be registered")
}

fn assert_runtime_mismatch<'owner>(result: SilexResult<RouterContext<'owner>>) {
    assert!(matches!(
        result,
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::RuntimeMismatch,
        )))
    ));
}

fn with_owner_accesses<'owner, R>(
    source: &'owner OwnerHandle,
    target: &'owner OwnerHandle,
    f: impl FnOnce(OwnerAccess<'owner>, OwnerAccess<'owner>) -> R,
) -> R {
    f(source.access(), target.access())
}

#[test]
fn foreign_search_is_rejected_before_query_computed_creation() {
    let mut source_runtime = Runtime::new();
    let source_root = source_runtime
        .owner()
        .expect("source root should be created");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime
        .owner()
        .expect("target root should be created");

    with_owner_accesses(&source_root, &target_root, |source, target| {
        let foreign_search_signal = source
            .signal(String::from("?foreign=true"))
            .expect("foreign search signal should be created");
        let path_signal = target
            .signal(String::from("/"))
            .expect("path signal should be created");
        let search_signal = target
            .signal(String::new())
            .expect("search signal should be created");
        let error_handler = test_handler(target);
        let result = RouterContext::new(
            SilexContext::new(target, error_handler.view()),
            RouterContextProps {
                base_path: String::from("/"),
                path: path_signal.read_signal(),
                search: foreign_search_signal.read_signal(),
                set_path: path_signal.write_signal(),
                set_search: search_signal.write_signal(),
            },
        );

        assert_runtime_mismatch(result);
    });
}

#[test]
fn foreign_write_destination_is_rejected_before_ctx_creation() {
    let mut source_runtime = Runtime::new();
    let source_root = source_runtime
        .owner()
        .expect("source root should be created");
    let mut target_runtime = Runtime::new();
    let target_root = target_runtime
        .owner()
        .expect("target root should be created");

    with_owner_accesses(&source_root, &target_root, |source, target| {
        let foreign_path_signal = source
            .signal(String::from("/foreign"))
            .expect("foreign path signal should be created");
        let foreign_set_path = foreign_path_signal.write_signal();
        let path_signal = target
            .signal(String::from("/"))
            .expect("path signal should be created");
        let search_signal = target
            .signal(String::new())
            .expect("search signal should be created");
        let error_handler = test_handler(target);
        let result = RouterContext::new(
            SilexContext::new(target, error_handler.view()),
            RouterContextProps {
                base_path: String::from("/"),
                path: path_signal.read_signal(),
                search: search_signal.read_signal(),
                set_path: foreign_set_path,
                set_search: search_signal.write_signal(),
            },
        );

        assert_runtime_mismatch(result);
    });
}
