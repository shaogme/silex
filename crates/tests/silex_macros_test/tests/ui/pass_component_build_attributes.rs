#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuildAttributes<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
) -> impl View<'scope> {
    let _ = scope;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let mut view = BuildAttributes(scope, AnyView::Empty)
            .class("before")
            .build()
            .class("after");
        view.apply_attributes(Vec::new());
        let _ = AnyView::new(view);
    });
}
