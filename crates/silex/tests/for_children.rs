#[test]
fn children_closures_receive_contextual_item_types() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/pass_for_children_field_access.rs");
}
