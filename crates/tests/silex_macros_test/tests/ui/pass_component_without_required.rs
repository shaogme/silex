#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn WithoutRequired<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain(default)] disabled: bool,
) -> impl View<'scope> {
    let _ = (scope, disabled);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let view = WithoutRequired(scope, AnyView::Empty).build();
        let _ = AnyView::new(view);
    });
}
