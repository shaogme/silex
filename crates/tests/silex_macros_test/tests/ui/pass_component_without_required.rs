#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn WithoutRequired<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
    #[chain(default)] disabled: bool,
) -> impl View<'scope> {
    let _ = (scope, disabled);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let view = WithoutRequired(scope, AnyView::Empty)
            .error_handler(error_handler)
            .build();
        let _ = AnyView::new(view);
    });
}
