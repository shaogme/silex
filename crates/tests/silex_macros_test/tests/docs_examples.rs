#![deny(warnings)]

#[path = "../../../../docs/examples/silex_rx/basic.rs"]
mod basic;

#[test]
fn rx_documentation_example_compiles() {
    assert!(basic::run().is_ok());
}
