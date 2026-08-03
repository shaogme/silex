use silex_core::Runtime;
use silex_dom::{element::mount_to_body, view::AnyView};

fn main() {
    let mut runtime = Runtime::new();
    let _root = runtime.run(|scope| {
        let value = String::from("borrowed-view");
        let view = AnyView::new(value.as_str());
        mount_to_body(scope, view);
    });
}
