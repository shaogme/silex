use silex_router::{PathTail, macros::routes};

fn main() {
    let routes = routes!(AppRoutes {
        home "/" => move |_ctx| silex_router::dom::view::AnyView::from("home"),
        user "/users/:id" => move |_ctx, id: u32| {
            silex_router::dom::view::AnyView::from(id.to_string())
        },
        files "/files/*rest" => move |_ctx, rest: PathTail| {
            silex_router::dom::view::AnyView::from(rest.into_inner())
        },
        fallback "/*" => move |_ctx| silex_router::dom::view::AnyView::from("not found"),
    });

    let _ = routes.table();
    let _ = routes.user(42);
    let _ = routes.files(PathTail::from("docs/reference"));
}
