#[test]
fn net_scope_contract() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass_*.rs");
    tests.compile_fail("tests/ui/fail_child_connection_escape.rs");
    #[cfg(not(feature = "json"))]
    tests.compile_fail("tests/ui/fail_child_http_resource_escape.rs");
    #[cfg(feature = "json")]
    tests.compile_fail("tests/ui/fail_child_http_resource_escape_json.rs");
    tests.compile_fail("tests/ui/fail_foreign_source_declaration.rs");
    tests.compile_fail("tests/ui/fail_old_net_constructors.rs");
    tests.pass("tests/ui/pass_scoped_callback_in_detached_task.rs");
    #[cfg(all(not(feature = "persist"), not(feature = "json")))]
    tests.compile_fail("tests/ui/fail_cache_without_persist.rs");
    #[cfg(all(not(feature = "persist"), feature = "json"))]
    tests.compile_fail("tests/ui/fail_cache_without_persist_json.rs");
    #[cfg(not(any(feature = "persist", feature = "json")))]
    tests.compile_fail("tests/ui/fail_scoped_builder_static.rs");
    #[cfg(all(feature = "persist", not(feature = "json")))]
    tests.compile_fail("tests/ui/fail_scoped_builder_static_persist.rs");
    #[cfg(all(feature = "persist", feature = "json"))]
    tests.compile_fail("tests/ui/fail_scoped_builder_static_json_persist.rs");
}
