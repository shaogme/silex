use silex_router::dom::view::AnyView;
use silex_router::dom::view::View;
use silex_router::macros::{component, router};
use silex_router::RouterContext;

#[component]
fn Home<'scope>(#[ctx] _ctx: RouterContext<'scope>) -> impl silex_router::dom::view::View<'scope> {
    "home"
}

router! {
    enum AppRoute {
        Home => "/",
    }
}

fn main() {
    let _ = AppRoute::table(|route, ctx| match route {
        AppRoute::Home => Home(ctx).build().into_any(),
    });
    let _ = AnyView::from("component");
}
