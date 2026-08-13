use silex_dom::view::DynamicRenderer;

fn make_renderer<'scope>(value: &'scope str) -> DynamicRenderer<'static> {
    DynamicRenderer::new(move |_| {
        let _ = value.len();
        Ok(())
    })
}

fn main() {
    let value = String::from("scoped-renderer");
    let _renderer = make_renderer(&value);
}
