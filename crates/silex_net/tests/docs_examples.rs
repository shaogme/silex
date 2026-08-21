#![deny(warnings)]

#[path = "../../../docs/examples/silex_net/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented net example should run");
}
