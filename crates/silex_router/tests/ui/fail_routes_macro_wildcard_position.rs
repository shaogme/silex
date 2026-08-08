use silex_router::macros::routes;

fn main() {
    let _ = routes!(WildcardPosition {
        files "/files/*rest/more" => move |_ctx| {
            silex_router::dom::view::AnyView::from("invalid")
        },
    });
}
