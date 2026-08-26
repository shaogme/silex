use silex_router::core::{OwnerAccess, SilexContext};
use silex_router::macros::router;
use silex_router::{Link, RoutePath, Router, RouterContext, RouterContextProps};
use silex_view::attributes::GlobalEventAttributes;
use silex_view::elements::AnyView;

router! {
    enum AppRoute {
        Home => "/",
    }
}

fn compile_owner_api<'owner>(owner: OwnerAccess<'owner>) {
    let text = owner
        .signal(String::from("scoped"))
        .expect("text signal should be created");
    let path = owner
        .signal(String::from("/"))
        .expect("path signal should be created");
    let search = owner
        .signal(String::new())
        .expect("search signal should be created");
    let error_handler = owner
        .error_handler(|_| {})
        .expect("error handler should be registered");
    let silex = SilexContext::new(owner, error_handler.view());
    let ctx = RouterContext::new(
        silex,
        RouterContextProps {
            base_path: String::from("/app"),
            path: path.read_signal(),
            search: search.read_signal(),
            set_path: path.write_signal(),
            set_search: search.write_signal(),
        },
    )
    .expect("router ctx should be created");
    let _link = Link(
        ctx,
        RoutePath::new("/").expect("route path should be valid"),
    )
    .children(text)
    .active_class("active")
    .on_click(|_| Ok(()))
    .build();
    let table = AppRoute::table(|route, _ctx| match route {
        AppRoute::Home => AnyView::from("home"),
    })
    .expect("route table should compile");
    let _router = Router(silex).base("/app").routes(table).build();
}

fn main() {
    let _ = compile_owner_api;
}
