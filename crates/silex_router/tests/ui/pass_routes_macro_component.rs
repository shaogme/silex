use silex_router::macros::{component, router};
use silex_router::RouterContext;
use silex_view::{AnyView, View};

#[component]
fn Home<'scope>(#[ctx] _ctx: RouterContext<'scope>) -> impl silex_view::View<'scope> {
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
