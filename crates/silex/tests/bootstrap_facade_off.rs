#![cfg(not(feature = "bootstrap"))]

#[test]
fn bootstrap_facade_requires_the_bootstrap_feature() {
    trybuild::TestCases::new().compile_fail("tests/ui/fail_bootstrap_facade_off.rs");
}
