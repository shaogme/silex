use silex_router::core::Scope;
use silex_router::dom::attribute::GlobalEventAttributes;
use silex_router::dom::view::AnyView;
use silex_router::macros::routes;
use silex_router::{Link, RoutePath, Router, RouterContext, RouterContextProps};

fn compile_scoped_api<'scope>(scope: Scope<'scope>) {
    let (text, _) = scope
        .signal(String::from("scoped"))
        .expect("text signal should be created");
    let (path, set_path) = scope
        .signal(String::from("/"))
        .expect("path signal should be created");
    let (search, set_search) = scope
        .signal(String::new())
        .expect("search signal should be created");
    let context = RouterContext::new(
        scope,
        RouterContextProps {
            base_path: String::from("/app"),
            path,
            search,
            set_path,
            set_search,
        },
        scope
            .error_handler(|_| {})
            .expect("error handler should be registered"),
    )
    .expect("router context should be created");
    let _link = Link(
        context,
        RoutePath::new("/").expect("route path should be valid"),
    )
    .error_handler(
        scope
            .error_handler(|_| {})
            .expect("error handler should be registered"),
    )
    .children(text)
    .active_class("active")
    .on_click(|_| Ok(()))
    .build();
    let routes = routes!(AppRoutes {
        home "/" => move |_ctx| AnyView::from("home"),
    })
    .expect("route catalog should compile");
    let _router = Router(
        scope,
        scope
            .error_handler(|_| {})
            .expect("error handler should be registered"),
    )
    .base("/app")
    .routes(routes.table())
    .build();
}

fn main() {
    let _ = compile_scoped_api;
}
