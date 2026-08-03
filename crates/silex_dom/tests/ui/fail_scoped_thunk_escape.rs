use silex_dom::view::RenderThunk;

fn make_renderer<'scope>(value: &'scope str) -> RenderThunk<'static> {
    RenderThunk::new(move |_| {
        let _ = value.len();
    })
}

fn main() {
    let value = String::from("scoped-renderer");
    let _renderer = make_renderer(&value);
}
