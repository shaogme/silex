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
