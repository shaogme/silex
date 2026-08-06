use silex_router::dom::view::AnyView;
use silex_router::macros::{component, Route};
use silex_router::{RouteView, RouterContext};

#[component]
fn StaticView<'scope>() -> AnyView<'scope> {
    AnyView::from("static")
}

#[derive(Clone, PartialEq, Route)]
enum AppRoute {
    #[route("/", view = StaticView)]
    Home,
}

fn main() {
    let _render: for<'scope> fn(
        &AppRoute,
        RouterContext<'scope>,
    ) -> AnyView<'scope> = AppRoute::render;
}
