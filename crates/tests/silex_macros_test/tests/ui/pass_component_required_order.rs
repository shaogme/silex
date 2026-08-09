#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn RequiredOrder<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] first: String,
    #[chain] second: String,
) -> impl View<'scope> {
    let _ = (scope, first, second);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let view = RequiredOrder(scope, AnyView::Empty)
            .second(String::from("second"))
            .first(String::from("first"))
            .second(String::from("override"))
            .build();
        let _ = AnyView::new(view);
    });
}
