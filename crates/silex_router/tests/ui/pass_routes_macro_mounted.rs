use silex_router::dom::view::AnyView;
use silex_router::macros::routes;

fn main() {
    let users = routes!(UsersRoutes {
        list "/" => move |_ctx| AnyView::from("list"),
        detail "/:id" => move |_ctx, id: u32| AnyView::from(id.to_string()),
    })
    .expect("route catalog should compile")
    .at("/users")
    .expect("mount prefix should be valid");

    let app = routes!(AppRoutes {
        home "/" => move |_ctx| AnyView::from("home"),
    })
    .expect("route catalog should compile");

    let _ = users.list();
    let _ = users.detail(42);
    let _ = app
        .table()
        .nest(users.prefix(), users.table(), move |_ctx, outlet| outlet)
        .expect("nested route table should compile");
}
