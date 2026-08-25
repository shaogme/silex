#[path = "../../../docs/examples/silex_dom/ssr.rs"]
mod ssr;

#[test]
fn ssr_documentation_example_runs() {
    assert!(ssr::run().is_ok());
}
