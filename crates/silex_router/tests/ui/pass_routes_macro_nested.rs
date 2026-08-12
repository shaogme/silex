use silex_router::PathTail;
use silex_router::dom::view::AnyView;
use silex_router::macros::routes;

fn main() {
    let routes = routes!(AppRoutes {
        home "/" => move |_ctx| AnyView::from("home"),
        nest users "/users" => move |_ctx, outlet| { outlet } {
            list "/" => move |_ctx| AnyView::from("list"),
            detail "/:id" => move |_ctx, id: u32| AnyView::from(id.to_string()),
            files "/files/*rest" => move |_ctx, rest: PathTail| {
                AnyView::from(rest.into_inner())
            },
        },
    })
    .expect("route catalog should compile");

    let _ = routes.table();
    let _ = routes.users().list();
    let _ = routes.users().detail(42);
    let _ = routes.users().files(PathTail::from("docs/reference"));
}
