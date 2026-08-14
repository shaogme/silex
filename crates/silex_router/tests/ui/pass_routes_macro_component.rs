use silex_router::dom::view::AnyView;
use silex_router::dom::view::View;
use silex_router::macros::{component, router};
use silex_router::RouterContext;

#[component]
fn Home<'scope>(#[context] _context: RouterContext<'scope>) -> impl silex_router::dom::view::View<'scope> {
    "home"
}

router! {
    enum AppRoute {
        Home => "/",
    }
}

fn main() {
    let _ = AppRoute::table(|route, context| match route {
        AppRoute::Home => Home(context).build().into_any(),
    });
    let _ = AnyView::from("component");
}
