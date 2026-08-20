#![deny(warnings)]

#[path = "../../../docs/examples/silex_core/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented core example should run");
}
