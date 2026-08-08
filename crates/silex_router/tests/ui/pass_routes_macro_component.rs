use silex_router::dom::view::AnyView;
use silex_router::macros::{component, routes};
use silex_router::RouterContext;

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
            UserView(ctx, id).label("detail")
        },
    });

    let _ = routes.table();
}
