#[cfg(not(miri))]
#[test]
fn compile_fail() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
