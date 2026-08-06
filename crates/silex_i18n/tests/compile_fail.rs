#[test]
fn scoped_i18n_api_rejects_legacy_constructors() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass_*.rs");
    tests.compile_fail("tests/ui/fail_*.rs");
}

#[cfg(feature = "browser-tests")]
#[test]
fn scoped_translation_view_rejects_child_escape() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/view_translation_escape.rs");
}
