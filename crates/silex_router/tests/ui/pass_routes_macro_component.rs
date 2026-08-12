use silex_router::RouterContext;
use silex_router::dom::view::AnyView;
use silex_router::macros::{component, routes};

#[component]
fn UserView<'scope>(
    _ctx: RouterContext<'scope>,
    id: u32,
    #[chain] label: String,
) -> AnyView<'scope> {
    AnyView::from(format!("{label}:{id}"))
}

fn main() {
    let routes = routes!(AppRoutes {
        user "/users/:id" => move |ctx, id: u32| {
            UserView(ctx, id).label("detail").build()
        },
    })
    .expect("route catalog should compile");

    let _ = routes.table();
}
