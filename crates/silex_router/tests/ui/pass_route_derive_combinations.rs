#![allow(non_snake_case)]

use silex_router::dom::view::{AnyView, View};
use silex_router::macros::{component, Route};
use silex_router::{RouteView, RouterContext, Routable};

#[component]
fn FirstGuard<'scope>(children: AnyView<'scope>) -> impl View<'scope> {
    children
}

#[component]
fn SecondGuard<'scope>(children: AnyView<'scope>) -> impl View<'scope> {
    children
}

#[component]
fn DetailView<'scope>(
    ctx: RouterContext<'scope>,
    account: String,
    #[chain] project: u32,
    #[chain] slug: String,
    #[chain] route: ChildRoute,
) -> impl View<'scope> {
    let _ = (ctx, account, project, slug, route);
    AnyView::from("detail")
}

#[derive(Clone, PartialEq, Route)]
enum ChildRoute {
    #[route("/")]
    Index,
    #[route("/settings/:tab")]
    Settings { tab: String },
}

#[derive(Clone, PartialEq, Route)]
enum AppRoute {
    #[route(
        "/account/:account/project/:project/:slug/*",
        view = DetailView,
        pass_ctx = true,
        guard = [FirstGuard, SecondGuard]
    )]
    Detail {
        account: String,
        project: u32,
        slug: String,
        #[nested]
        route: ChildRoute,
    },
    #[route("/*")]
    NotFound,
}

fn main() {
    let route = AppRoute::match_path("/account/acme/project/7/intro/settings/access")
        .expect("route with guards and multiple fields should match");
    assert_eq!(
        route.to_path(),
        "/account/acme/project/7/intro/settings/access"
    );
    let _render: for<'scope> fn(
        &AppRoute,
        RouterContext<'scope>,
    ) -> AnyView<'scope> = AppRoute::render;
}
