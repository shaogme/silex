#![allow(non_snake_case)]

use silex_router::dom::view::{AnyView, View};
use silex_router::macros::{component, Route};
use silex_router::{RouteView, RouterContext, Routable};

fn ChildView<'scope>() -> AnyView<'scope> {
    AnyView::from("child")
}

#[derive(Clone, PartialEq, Route)]
enum ChildRoute {
    #[route("/", view = ChildView)]
    Index,
}

#[component]
fn ParentView<'scope>(
    ctx: RouterContext<'scope>,
    id: u32,
    #[chain]
    route: ChildRoute,
) -> impl View<'scope> {
    let _ = (ctx, id, route);
    AnyView::from("parent")
}

#[derive(Clone, PartialEq, Route)]
enum ParentRoute {
    #[route("/parent/:id/*", view = ParentView, pass_ctx = true)]
    Parent {
        id: u32,
        #[nested]
        route: ChildRoute,
    },
}

fn main() {
    let route = ParentRoute::match_path("/parent/7/").expect("nested route should match");
    let _ = route.to_path();
    let _render: for<'scope> fn(
        &ParentRoute,
        RouterContext<'scope>,
    ) -> AnyView<'scope> = ParentRoute::render;
}
