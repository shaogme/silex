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

#[test]
fn nested_routes_build_prefixed_catalog_paths_and_match_child_routes() {
    let routes = routes!(NestedRoutes {
        home "/" => move |_ctx| silex_router::dom::view::AnyView::from("home"),
        nest users "/users" => move |_ctx, outlet| { outlet } {
            list "/" => move |_ctx| silex_router::dom::view::AnyView::from("list"),
            detail "/:id" => move |_ctx, id: u32| {
                silex_router::dom::view::AnyView::from(id.to_string())
            },
        },
    });

    assert_eq!(routes.home().as_str(), "/");
    assert_eq!(routes.users().list().as_str(), "/users");
    assert_eq!(routes.users().detail(42).as_str(), "/users/42");

    let child_table = routes.users().table();
    let matched = child_table.match_path("/42").unwrap();
    assert_eq!(matched.parse::<u32>("id").unwrap(), 42);
}

#[test]
fn nested_routes_compose_recursively() {
    let routes = routes!(NestedRoutes {
        nest admin "/admin" => move |_ctx, outlet| { outlet } {
            nest users "/users" => move |_ctx, outlet| { outlet } {
                detail "/:id" => move |_ctx, id: u32| {
                    silex_router::dom::view::AnyView::from(id.to_string())
                },
            },
        },
    });

    assert_eq!(routes.admin().users().detail(7).as_str(), "/admin/users/7");
    assert_eq!(
        routes
            .admin()
            .users()
            .table()
            .match_path("/7")
            .unwrap()
            .parse::<u32>("id")
            .unwrap(),
        7
    );
}

#[test]
fn standalone_catalog_mounts_with_prefixed_typed_paths() {
    let users = routes!(UsersRoutes {
        list "/" => move |_ctx| silex_router::dom::view::AnyView::from("list"),
        detail "/:id" => move |_ctx, id: u32| {
            silex_router::dom::view::AnyView::from(id.to_string())
        },
    })
    .at("/users");

    assert_eq!(users.prefix().as_str(), "/users");
    assert_eq!(users.list().as_str(), "/users");
    assert_eq!(users.detail(42).as_str(), "/users/42");

    let app = routes!(AppRoutes {
        home "/" => move |_ctx| silex_router::dom::view::AnyView::from("home"),
    });
    let table = app
        .table()
        .nest(users.prefix(), users.table(), move |_ctx, outlet| outlet);

    assert!(table.match_path("/users/42").is_some());
    assert_eq!(users.table().match_path("/42").unwrap().route_id(), 1);
}

#[test]
fn mounted_catalog_preserves_recursive_child_prefixes() {
    let routes = routes!(AdminRoutes {
        nest users "/users" => move |_ctx, outlet| { outlet } {
            detail "/:id" => move |_ctx, id: u32| {
                silex_router::dom::view::AnyView::from(id.to_string())
            },
        },
    })
    .at("/admin");

    assert_eq!(routes.users().detail(7).as_str(), "/admin/users/7");
    assert!(routes.table().match_path("/users/7").is_some());
}
