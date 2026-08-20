#![deny(warnings)]

#[path = "../../../docs/examples/silex_reactivity/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented reactive example should run");
}
