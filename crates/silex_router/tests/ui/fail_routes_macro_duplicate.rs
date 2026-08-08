use silex_router::macros::routes;

fn main() {
    let _ = routes!(DuplicateRoutes {
        first "/users/:id" => move |_ctx, id: u32| {
            silex_router::dom::view::AnyView::from(id)
        },
        second "/users/:name" => move |_ctx, name: u32| {
            silex_router::dom::view::AnyView::from(name)
        },
    });
}
