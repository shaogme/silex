#![deny(warnings)]

#[path = "../../../docs/examples/silex_router/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented router example should compile and run");
}
