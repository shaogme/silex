use silex_router::{PathTail, RouteMatcher, macros::router};
use silex_view::elements::AnyView;

router! {
    pub enum CssRoute {
        Basics => "/",
        Theming => "/theming",
    }
}

router! {
    pub enum AppRoute {
        Home => "/",
        User { id: u32 } => "/users/:id",
        Files { rest: PathTail } => "/files/*rest",
        Css(CssRoute) {
            prefix: "/css";
            layout: |_ctx, outlet| outlet;
        },
        NotFound => "/*",
    }
}

router! {
    enum PriorityRoute {
        Static => "/items/new",
        Parameter { id: u32 } => "/items/:id",
        Wildcard { rest: PathTail } => "/items/*rest",
        Fallback => "/*",
    }
}

router! {
    enum LeafRoute {
        Home => "/",
        Detail { id: u32 } => "/:id",
        Fallback => "/*",
    }
}

router! {
    enum MiddleRoute {
        Users(LeafRoute) {
            prefix: "/users";
            layout: |_ctx, outlet| outlet;
        },
        Fallback => "/*",
    }
}

router! {
    enum RootRoute {
        App(MiddleRoute) {
            prefix: "/app";
            layout: |_ctx, outlet| outlet;
        },
        Fallback => "/*",
    }
}

#[test]
fn router_macro_generates_typed_paths_and_matchers() {
    assert_eq!(AppRoute::Home.path().expect("home path").as_str(), "/");
    assert_eq!(
        AppRoute::User { id: 42 }
            .path()
            .expect("user path")
            .as_str(),
        "/users/42"
    );
    assert_eq!(
        AppRoute::Files {
            rest: PathTail::from("docs/reference"),
        }
        .path()
        .expect("files path")
        .as_str(),
        "/files/docs/reference"
    );
    assert_eq!(
        AppRoute::Css(CssRoute::Theming)
            .path()
            .expect("nested path")
            .as_str(),
        "/css/theming"
    );
}

#[test]
fn router_macro_decodes_enum_values_and_falls_back_after_decode_failure() {
    let css_routes = CssRoute::compile().expect("child route patterns should compile");
    let app_routes = AppRoute::compile().expect("route patterns should compile");

    assert!(matches!(
        css_routes.match_path("/theming").expect("child path"),
        Some(CssRoute::Theming)
    ));
    assert!(matches!(
        app_routes.match_path("/users/42").expect("valid path"),
        Some(AppRoute::User { id: 42 })
    ));
    assert!(matches!(
        app_routes
            .match_path("/files/a%2Fb/c")
            .expect("valid path"),
        Some(AppRoute::Files { rest }) if rest.as_str() == "a/b/c"
    ));
    assert!(matches!(
        app_routes
            .match_path("/users/not-a-number")
            .expect("fallback path"),
        Some(AppRoute::NotFound)
    ));
}

#[test]
fn router_macro_compiles_and_reuses_a_typed_matcher() {
    let routes: AppRouteMatcher = AppRoute::compile().expect("route patterns should compile");

    assert!(matches!(
        routes.match_path("/users/42").expect("valid path"),
        Some(AppRoute::User { id: 42 })
    ));
    assert!(matches!(
        routes
            .match_path("/users/not-a-number")
            .expect("fallback path"),
        Some(AppRoute::NotFound)
    ));
    assert!(matches!(
        routes.match_path("/css/theming").expect("nested path"),
        Some(AppRoute::Css(CssRoute::Theming))
    ));
    assert!(routes.match_path("/users//").is_err());
}

#[test]
fn router_macro_exposes_patterns_for_a_reused_raw_matcher() {
    assert_eq!(
        AppRoute::patterns(),
        &["/", "/users/:id", "/files/*rest", "/css/*", "/*"]
    );
    let matcher = RouteMatcher::from_patterns(AppRoute::patterns())
        .expect("generated patterns should compile");
    assert!(matcher.match_path("/files/a%2Fb/c").is_some());
}

#[test]
fn router_macro_preserves_static_parameter_and_wildcard_priority() {
    let routes = PriorityRoute::compile().expect("route patterns should compile");

    assert!(matches!(
        routes.match_path("/items/new").expect("static path"),
        Some(PriorityRoute::Static)
    ));
    assert!(matches!(
        routes.match_path("/items/42").expect("parameter path"),
        Some(PriorityRoute::Parameter { id: 42 })
    ));
    assert!(matches!(
        routes
            .match_path("/items/a/b")
            .expect("wildcard path"),
        Some(PriorityRoute::Wildcard { rest }) if rest.as_str() == "a/b"
    ));
}

#[test]
fn router_macro_preserves_nested_prefix_child_fallback_and_multiple_levels() {
    let routes = RootRoute::compile().expect("nested route patterns should compile");

    assert!(matches!(
        routes
            .match_path("/app/users/42")
            .expect("nested parameter path"),
        Some(RootRoute::App(MiddleRoute::Users(LeafRoute::Detail {
            id: 42
        })))
    ));
    assert!(matches!(
        routes
            .match_path("/app/users/not-a-number")
            .expect("child fallback path"),
        Some(RootRoute::App(MiddleRoute::Users(LeafRoute::Fallback)))
    ));
    assert!(matches!(
        routes
            .match_path("/app/other")
            .expect("middle fallback path"),
        Some(RootRoute::App(MiddleRoute::Fallback))
    ));
    assert!(matches!(
        routes.match_path("/outside").expect("root fallback path"),
        Some(RootRoute::Fallback)
    ));
}

#[test]
fn independent_child_enum_builds_its_own_route_table() {
    let table = CssRoute::table(|route, _ctx| match route {
        CssRoute::Basics => AnyView::from("basics"),
        CssRoute::Theming => AnyView::from("theming"),
    })
    .expect("child route table should compile");

    assert_eq!(
        CssRoute::Theming.path().expect("child path").as_str(),
        "/theming"
    );
    assert!(table.match_path("/theming").is_some());
}

#[test]
fn router_macro_builds_a_route_table_from_an_exhaustive_view_match() {
    let table = AppRoute::table(|route, _ctx| match route {
        AppRoute::Home => AnyView::from("home"),
        AppRoute::User { id } => AnyView::from(id.to_string()),
        AppRoute::Files { rest } => AnyView::from(rest.into_inner()),
        AppRoute::Css(CssRoute::Basics) => AnyView::from("css basics"),
        AppRoute::Css(CssRoute::Theming) => AnyView::from("css theming"),
        AppRoute::NotFound => AnyView::from("not found"),
    })
    .expect("route table should compile");

    assert!(table.match_path("/css/theming").is_some());
    assert!(table.match_path("/files/a/b").is_some());
}
