#![deny(warnings)]

#[path = "../../../docs/examples/silex_i18n/basic.rs"]
mod basic;

#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented i18n example should run");
}
