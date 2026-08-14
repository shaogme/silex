use silex_router::dom::view::AnyView;
use silex_router::macros::router;
use silex_router::RouterContext;

router! {
    enum AppRoute {
        Home => "/",
    }
}

fn make_routes<'scope>(context: RouterContext<'scope>) {
    let _ = AppRoute::table(move |route, _context| match route {
        AppRoute::Home => {
            let _ = context;
            AnyView::from("home")
        }
    });
}

fn main() {
    let _ = make_routes;
}
