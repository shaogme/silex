use silex_router::RouterContext;
use silex_router::dom::view::AnyView;
use silex_router::macros::{component, routes};

#[component]
fn Home<'scope>(
    #[context] _context: RouterContext<'scope>,
) -> AnyView<'scope> {
    AnyView::from("home")
}

fn make_routes<'scope>(context: RouterContext<'scope>) {
    let routes = routes!(AppRoutes {
        home "/" => move |ctx| Home(ctx).build(),
    })
    .expect("route catalog should compile");
    let _ = routes.table();
    let _ = context;
}

fn main() {
    let _ = make_routes;
}
