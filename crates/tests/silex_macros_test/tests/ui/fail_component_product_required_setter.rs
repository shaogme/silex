#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn ProductRequiredSetter<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] mandatory: String,
) -> impl View<'scope> {
    let _ = (scope, mandatory);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let product = ProductRequiredSetter(scope, AnyView::Empty)
            .mandatory(String::from("value"))
            .build();
        let _ = product.mandatory(String::from("replacement"));
    });
}
