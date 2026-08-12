use silex_router::RouterContext;
use silex_core::ErrorReporter;
use silex_router::dom::view::AnyView;
use silex_router::macros::{component, routes};

#[component]
fn UserView<'scope>(
    _ctx: RouterContext<'scope>,
    id: u32,
    #[chain] label: String,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> AnyView<'scope> {
    let _ = error_handler;
    AnyView::from(format!("{label}:{id}"))
}

fn main() {
    let routes = routes!(AppRoutes {
        user "/users/:id" => move |ctx, id: u32| {
            let error_handler = ctx.scope().error_handler(|_| {})?;
            UserView(ctx, id)
                .error_handler(error_handler)
                .label("detail")
                .build()
        },
    })
    .expect("route catalog should compile");

    let _ = routes.table();
}
