use silex_router::dom::view::{AnyView, View};
use silex_router::macros::component;

#[component]
fn MissingScope(children: AnyView) -> impl View<'static> {
    children
}

fn main() {}
