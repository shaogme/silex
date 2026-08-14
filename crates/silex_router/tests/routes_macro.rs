use silex_router::{PathTail, dom::view::AnyView, macros::router};

router! {
    pub enum AppRoute {
        Home => "/",
        User { id: u32 } => "/users/:id",
        Files { rest: PathTail } => "/files/*rest",
        Css {
            prefix: "/css";
            layout: |_context, outlet| outlet;
            children: {
                Basics => "/",
                Theming => "/theming",
            }
        },
        NotFound => "/*",
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
    assert!(matches!(
        AppRoute::match_path("/users/42").expect("valid path"),
        Some(AppRoute::User { id: 42 })
    ));
    assert!(matches!(
        AppRoute::match_path("/files/a%2Fb/c").expect("valid path"),
        Some(AppRoute::Files { rest }) if rest.as_str() == "a/b/c"
    ));
    assert!(matches!(
        AppRoute::match_path("/users/not-a-number").expect("fallback path"),
        Some(AppRoute::NotFound)
    ));
}

#[test]
fn router_macro_builds_a_route_table_from_an_exhaustive_view_match() {
    let table = AppRoute::table(|route, _context| match route {
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
