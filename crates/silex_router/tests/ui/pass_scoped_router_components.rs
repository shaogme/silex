use silex_router::dom::view::AnyView;
use silex_router::dom::attribute::GlobalEventAttributes;
use silex_router::macros::routes;
use silex_router::{Link, RoutePath, Router, RouterContext, RouterContextProps};
use silex_router::core::Scope;

fn compile_scoped_api<'scope>(scope: Scope<'scope>) {
    let (text, _) = scope.signal(String::from("scoped"));
    let (path, set_path) = scope.signal(String::from("/"));
    let (search, set_search) = scope.signal(String::new());
    let context = RouterContext::try_new(
        scope,
        RouterContextProps {
            base_path: String::from("/app"),
            path,
            search,
            set_path,
            set_search,
        },
    )
    .expect("router context should be created");
    let _link = Link(context, RoutePath::new("/").unwrap())
        .children(text)
        .active_class("active")
        .on_click(|_| Ok(()))
        .build();
    let routes = routes!(AppRoutes {
        home "/" => move |_ctx| AnyView::from("home"),
    });
    let _router = Router(scope).base("/app").routes(routes.table()).build();
}

fn main() {
    let _ = compile_scoped_api;
}
