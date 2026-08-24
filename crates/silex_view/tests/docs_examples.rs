#[path = "../../../docs/examples/silex_dom/basic.rs"]
mod dom_basic;

#[test]
fn dom_documentation_example_compiles_on_native() {
    dom_basic::run().expect("the documented view example should compile and run");
}
