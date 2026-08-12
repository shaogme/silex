#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuiltProduct<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
    #[chain] label: String,
) -> impl View<'scope> {
    let _ = (scope, label);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let view = BuiltProduct(scope, AnyView::Empty)
            .error_handler(error_handler)
            .label(String::from("Save"))
            .build();
        let _ = AnyView::new(view);
    });
}
