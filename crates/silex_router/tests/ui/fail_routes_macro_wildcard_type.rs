use silex_router::macros::routes;

fn main() {
    let _ = routes!(WrongWildcard {
        files "/files/*rest" => move |_ctx, rest: String| {
            silex_router::dom::view::AnyView::from(rest)
        },
    });
}
