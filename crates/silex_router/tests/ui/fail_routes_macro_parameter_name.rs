use silex_router::macros::routes;

fn main() {
    let _ = routes!(WrongName {
        user "/users/:id" => move |_ctx, user_id: u32| {
            silex_router::dom::view::AnyView::from(user_id)
        },
    });
}
