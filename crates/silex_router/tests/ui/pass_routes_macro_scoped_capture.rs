use silex_core::ErrorReporter;
use silex_router::RouterContext;
use silex_router::dom::view::AnyView;
use silex_router::macros::{component, routes};

#[component]
fn Home<'scope>(
    ctx: RouterContext<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> AnyView<'scope> {
    let _ = ctx;
    let _ = error_handler;
    AnyView::from("home")
}

fn make_routes<'scope>(reporter: ErrorReporter<'scope>) {
    let routes = routes!(AppRoutes {
        home "/" => move |ctx| Home(ctx).error_handler(reporter.clone()),
    });
    let _ = routes.table();
}

fn main() {
    let _ = make_routes;
}
