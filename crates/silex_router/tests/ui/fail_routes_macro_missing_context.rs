use silex_router::macros::routes;

fn main() {
    let _ = routes!(MissingContext {
        user "/users/:id" => move |id: u32| silex_router::dom::view::AnyView::from(id),
    });
}
