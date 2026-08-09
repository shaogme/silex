use silex_router::macros::routes;

fn main() {
    let _ = routes!(DuplicateCompletePath {
        user "/users/:id" => move |_ctx, id: u32| {
            silex_router::dom::view::AnyView::from(id.to_string())
        },
        nest users "/users" => move |_ctx, outlet| { outlet } {
            detail "/:name" => move |_ctx, name: u32| {
                silex_router::dom::view::AnyView::from(name.to_string())
            },
        },
    });
}
