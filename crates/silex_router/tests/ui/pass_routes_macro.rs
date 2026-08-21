use silex_router::{
    PathTail,
    RouteMatcher,
    dom::view::AnyView,
    macros::router,
};

router! {
    enum AppRoute {
        Home => "/",
        User { id: u32 } => "/users/:id",
        Files { rest: PathTail } => "/files/*rest",
        Fallback => "/*",
    }
}

fn main() {
    let _ = AppRoute::Home.path();
    let _ = AppRoute::User { id: 42 }.path();
    let _ = AppRoute::Files {
        rest: PathTail::from("docs/reference"),
    }
    .path();
    let _ = AppRoute::compile().map(|routes| routes.match_path("/users/42"));
    let _ = RouteMatcher::from_patterns(AppRoute::patterns());
    let _ = AppRoute::table(|route, _ctx| match route {
        AppRoute::Home => AnyView::from("home"),
        AppRoute::User { id } => AnyView::from(id.to_string()),
        AppRoute::Files { rest } => AnyView::from(rest.into_inner()),
        AppRoute::Fallback => AnyView::from("fallback"),
    });
}
