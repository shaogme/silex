#![deny(warnings)]

#[path = "../../../docs/examples/silex/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented silex example should compile and run");
}
