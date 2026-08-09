#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuiltProduct<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] label: String,
) -> impl View<'scope> {
    let _ = (scope, label);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let view = BuiltProduct(scope, AnyView::Empty)
            .label(String::from("Save"))
            .build();
        let _ = AnyView::new(view);
    });
}
