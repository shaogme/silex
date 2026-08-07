use silex_core::Runtime;
use silex_dom::element::mount_to_body;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _ = mount_to_body(scope, "missing-handler");
    });
}
