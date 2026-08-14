use silex_router::{dom::view::AnyView, macros::router};

router! {
    enum AppRoute {
        Home => "/",
        Admin {
            prefix: "/admin";
            layout: |_context, outlet| outlet;
            children: {
                Users {
                    prefix: "/users";
                    layout: |_context, outlet| outlet;
                    children: {
                        Detail { id: u32 } => "/:id",
                    }
                },
            }
        },
    }
}

fn main() {
    let route = AppRoute::Admin(AdminRoute::Users(UsersRoute::Detail { id: 42 }));
    let _ = route.path();
    let _ = AppRoute::table(|route, _context| match route {
        AppRoute::Home => AnyView::from("home"),
        AppRoute::Admin(AdminRoute::Users(UsersRoute::Detail { id })) => {
            AnyView::from(id.to_string())
        }
    });
}
