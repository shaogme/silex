use silex_router::Routable;

#[derive(Clone, PartialEq, silex_router::macros::Route)]
enum RouteWithParentFallback {
    #[route("/users/:id")]
    User { id: u32 },
    #[route("/*")]
    NotFound,
}

#[derive(Clone, PartialEq, silex_router::macros::Route)]
enum RouteWithStaticChild {
    #[route("/foo/bar")]
    Bar,
    #[route("/*")]
    NotFound,
}

#[derive(Clone, PartialEq, silex_router::macros::Route)]
enum RouteWithParameterFallback {
    #[route("/foo/bar")]
    Bar,
    #[route("/:name")]
    Item { name: String },
}

#[derive(Clone, PartialEq, silex_router::macros::Route)]
enum NestedRoute {
    #[route("/")]
    Index,
    #[route("/settings/:tab")]
    Settings { tab: String },
}

#[derive(Clone, PartialEq, silex_router::macros::Route)]
enum ComplexRoute {
    #[route("/org/:org/user/:id/*")]
    User {
        org: String,
        id: u32,
        #[nested]
        route: NestedRoute,
    },
    #[route("/team/:team/*")]
    Team {
        #[nested]
        route: NestedRoute,
        team: String,
    },
    #[route("/article/:category/:slug")]
    Article { category: String, slug: String },
    #[route("/*")]
    NotFound,
}

#[test]
fn child_without_enough_segments_falls_back_to_parent_wildcard() {
    assert!(matches!(
        RouteWithParentFallback::match_path("/users"),
        Some(RouteWithParentFallback::NotFound)
    ));
}

#[test]
fn parameter_parse_failure_falls_back_to_parent_wildcard() {
    assert!(matches!(
        RouteWithParentFallback::match_path("/users/not-a-number"),
        Some(RouteWithParentFallback::NotFound)
    ));
}

#[test]
fn static_child_without_a_complete_path_falls_back_to_parent_wildcard() {
    assert!(matches!(
        RouteWithStaticChild::match_path("/foo"),
        Some(RouteWithStaticChild::NotFound)
    ));
}

#[test]
fn static_child_failure_falls_back_to_parent_parameter() {
    assert!(matches!(
        RouteWithParameterFallback::match_path("/foo"),
        Some(RouteWithParameterFallback::Item { name }) if name == "foo"
    ));
}

#[test]
fn route_derive_handles_multiple_params_nested_fields_and_to_path() {
    let route = ComplexRoute::match_path("/org/acme/user/7/settings/profile")
        .expect("complex nested route should match");

    assert!(matches!(
        &route,
        ComplexRoute::User {
            org,
            id,
            route: NestedRoute::Settings { tab },
        } if org == "acme" && *id == 7 && tab == "profile"
    ));
    assert_eq!(route.to_path(), "/org/acme/user/7/settings/profile");
}

#[test]
fn route_derive_accepts_nested_fields_before_params() {
    let route = ComplexRoute::match_path("/team/platform/settings/access")
        .expect("nested-first route should match");

    assert!(matches!(
        &route,
        ComplexRoute::Team {
            route: NestedRoute::Settings { tab },
            team,
        } if team == "platform" && tab == "access"
    ));
    assert_eq!(route.to_path(), "/team/platform/settings/access");
}

#[test]
fn route_derive_handles_multiple_plain_params_alongside_nested_routes() {
    let route = ComplexRoute::match_path("/article/reference/rust")
        .expect("multiple plain params should match");

    assert!(matches!(
        &route,
        ComplexRoute::Article { category, slug }
            if category == "reference" && slug == "rust"
    ));
    assert_eq!(route.to_path(), "/article/reference/rust");
}
