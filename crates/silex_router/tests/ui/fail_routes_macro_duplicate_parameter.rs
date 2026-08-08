use silex_router::macros::routes;

fn main() {
    let _ = routes!(DuplicateParameter {
        user "/users/:id/:id" => move |_ctx, id: u32, other: u32| {
            silex_router::dom::view::AnyView::from(id + other)
        },
    });
}
