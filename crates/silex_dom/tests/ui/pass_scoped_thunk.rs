use silex_dom::view::{DynamicRenderer, MountInstance};

fn make_renderer<'scope>(value: &'scope str) -> DynamicRenderer<'scope> {
    DynamicRenderer::new(move |_| {
        let _ = value.len();
        Ok(MountInstance::from_nodes(Vec::new()))
    })
}

fn main() {
    let value = String::from("scoped-renderer");
    let _renderer = make_renderer(&value);
}
