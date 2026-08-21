#![deny(warnings)]

#[path = "../../../docs/examples/silex_html/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented HTML example should compile and construct views");
}
