#[test]
fn macro_scope_contract() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass_macro_*.rs");
    tests.compile_fail("tests/ui/fail_macro_*.rs");
}
