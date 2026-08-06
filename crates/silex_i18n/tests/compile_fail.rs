#[test]
fn scoped_i18n_api_rejects_legacy_constructors() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass_*.rs");
    tests.compile_fail("tests/ui/fail_*.rs");
}
