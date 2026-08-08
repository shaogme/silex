use silex_router::macros::routes;

fn main() {
    let _ = routes!(MissingType {
        user "/users/:id" => move |_ctx, id| silex_router::dom::view::AnyView::from(id),
    });
}
