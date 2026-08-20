#![deny(warnings)]

#[path = "../../../docs/examples/silex_dom/basic.rs"]
mod basic;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn basic_documentation_example_runs() {
    basic::run().expect("the documented DOM example should compile and run");
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::basic;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn basic_documentation_example_runs() {
        basic::run().expect("the documented DOM example should mount and dispose");
    }
}
