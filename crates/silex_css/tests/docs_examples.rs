#[path = "../../../docs/examples/silex_css/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented CSS example should compile and run");
}
