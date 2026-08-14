use silex_router::{PathTail, dom::view::AnyView, macros::router};

router! {
    enum UsersRoute {
        List => "/",
        Detail { id: u32 } => "/:id",
        Files { rest: PathTail } => "/files/*rest",
    }
}

router! {
    enum AppRoute {
        Home => "/",
        Users(UsersRoute) {
            prefix: "/users";
            layout: |_ctx, outlet| outlet;
        },
    }
}

fn main() {
    let _ = AppRoute::Users(UsersRoute::List).path();
    let _ = AppRoute::Users(UsersRoute::Detail { id: 42 }).path();
    let _ = AppRoute::Users(UsersRoute::Files {
        rest: PathTail::from("docs/reference"),
    })
    .path();
    let _ = AppRoute::table(|route, _ctx| match route {
        AppRoute::Home => AnyView::from("home"),
        AppRoute::Users(UsersRoute::List) => AnyView::from("list"),
        AppRoute::Users(UsersRoute::Detail { id }) => AnyView::from(id.to_string()),
        AppRoute::Users(UsersRoute::Files { rest }) => AnyView::from(rest.into_inner()),
    });
}
