#[test]
fn app_host_builder_contract() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass_app_host_builder.rs");
    tests.compile_fail("tests/ui/fail_app_host_scope_escape.rs");
}
