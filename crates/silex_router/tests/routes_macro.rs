use silex_router::{PathTail, macros::routes};

fn identity_guard<'scope>(
    view: silex_router::dom::view::AnyView<'scope>,
) -> silex_router::dom::view::AnyView<'scope> {
    view
}

#[test]
fn routes_macro_builds_catalog_paths_and_table() {
    let routes = routes!(AppRoutes {
        home "/" => move |_ctx| silex_router::dom::view::AnyView::from("home"),
        settings "/users/settings" => move |_ctx| {
            silex_router::dom::view::AnyView::from("settings")
        },
        user "/users/:id" => move |_ctx, id: u32| {
            silex_router::dom::view::AnyView::from(id.to_string())
        },
        file "/file/:name" => move |_ctx, name: String| {
            silex_router::dom::view::AnyView::from(name)
        },
        guarded "/guarded" guards = [identity_guard] => move |_ctx| {
            silex_router::dom::view::AnyView::from("guarded")
        },
        guarded_after "/guarded-after" => move |_ctx| {
            silex_router::dom::view::AnyView::from("guarded after")
        }, guards = [identity_guard],
        files "/files/*rest" => move |_ctx, rest: PathTail| {
            silex_router::dom::view::AnyView::from(rest.into_inner())
        },
        fallback "/*" => move |_ctx| silex_router::dom::view::AnyView::from("not found"),
    });

    assert_eq!(routes.home().as_str(), "/");
    assert_eq!(routes.settings().as_str(), "/users/settings");
    assert_eq!(routes.user(42).as_str(), "/users/42");
    assert_eq!(routes.file(String::from("a/b")).as_str(), "/file/a%2Fb");
    assert_eq!(routes.files(PathTail::from("a/b")).as_str(), "/files/a/b");

    let table = routes.table();
    assert_eq!(table.match_path("/").unwrap().route_id(), 0);
    assert_eq!(
        table
            .match_path("/users/42")
            .unwrap()
            .parse::<u32>("id")
            .unwrap(),
        42
    );
    assert_eq!(
        table
            .match_path("/files/a%2Fb/c")
            .unwrap()
            .parse::<PathTail>("rest")
            .unwrap()
            .as_str(),
        "a/b/c"
    );
}
