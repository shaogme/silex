use silex_router::macros::routes;

fn main() {
    let _ = routes!(InvalidLayout {
        nest users "/users" => move |_ctx| { silex_router::dom::view::AnyView::from("users") } {
            list "/" => move |_ctx| silex_router::dom::view::AnyView::from("list"),
        },
    });
}
