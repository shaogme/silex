#[cfg_attr(miri, ignore)]
#[test]
fn typed_handles_reject_cross_kind_operations() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/*.rs");
}
