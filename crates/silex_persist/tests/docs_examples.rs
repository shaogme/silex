#![deny(warnings)]

#[path = "../../../docs/examples/silex_persist/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    assert!(basic::run().is_ok());
}
