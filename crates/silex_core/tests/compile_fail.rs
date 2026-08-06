#[test]
fn scoped_api_compile_failures() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass_*.rs");
    tests.compile_fail("tests/ui/fail_*.rs");
}
