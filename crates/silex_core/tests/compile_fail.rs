#[test]
fn scoped_api_compile_failures() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
