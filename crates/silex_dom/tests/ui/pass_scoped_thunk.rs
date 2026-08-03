use silex_dom::view::RenderThunk;

fn make_renderer<'scope, 'run>(value: &'scope str) -> RenderThunk<'scope, 'run> {
    RenderThunk::new(move |_| {
        let _ = value.len();
    })
}

fn main() {
    let value = String::from("scoped-renderer");
    let _renderer = make_renderer(&value);
}
