use silex_router::macros::routes;

fn main() {
    let _ = routes!(DynamicNestedPrefix {
        nest tenant "/:tenant" => move |_ctx, outlet| { outlet } {
            home "/" => move |_ctx| silex_router::dom::view::AnyView::from("home"),
        },
    });
}
