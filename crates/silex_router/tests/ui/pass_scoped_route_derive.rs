use silex_router::Routable;
use silex_router::dom::view::AnyView;

#[derive(Clone, PartialEq, silex_router::macros::Route)]
enum AppRoute {
    #[route("/")]
    Home,
}

fn main() {
    let route = <AppRoute as Routable>::match_path("/").expect("route should match");
    assert_eq!(route.to_path(), "/");
    let _render: for<'scope> fn(
        &AppRoute,
        silex_router::RouterContext<'scope>,
    ) -> AnyView<'scope> = <AppRoute as silex_router::RouteView>::render;
}
