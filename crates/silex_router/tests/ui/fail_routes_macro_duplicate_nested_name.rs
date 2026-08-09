use silex_router::macros::routes;

fn main() {
    let _ = routes!(DuplicateNestedName {
        nest users "/users" => move |_ctx, outlet| { outlet } {
            list "/" => move |_ctx| silex_router::dom::view::AnyView::from("list"),
        },
        nest users "/accounts" => move |_ctx, outlet| { outlet } {
            list "/" => move |_ctx| silex_router::dom::view::AnyView::from("accounts"),
        },
    });
}
