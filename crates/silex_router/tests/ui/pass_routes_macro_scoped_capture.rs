use silex_router::macros::router;
use silex_router::RouterContext;
use silex_view::AnyView;

router! {
    enum AppRoute {
        Home => "/",
    }
}

fn make_routes<'scope>(ctx: RouterContext<'scope>) {
    let _ = AppRoute::table(move |route, _ctx| match route {
        AppRoute::Home => {
            let _ = ctx;
            AnyView::from("home")
        }
    });
}

fn main() {
    let _ = make_routes;
}
