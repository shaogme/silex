#[test]
fn scope_escape_is_rejected() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/fail_*.rs");
}
