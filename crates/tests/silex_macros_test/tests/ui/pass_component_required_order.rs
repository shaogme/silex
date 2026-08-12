#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn RequiredOrder<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
    #[chain] first: String,
    #[chain] second: String,
) -> impl View<'scope> {
    let _ = (scope, first, second);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let view = RequiredOrder(scope, AnyView::Empty)
            .error_handler(error_handler)
            .second(String::from("second"))
            .first(String::from("first"))
            .second(String::from("override"))
            .build();
        let _ = AnyView::new(view);
    });
}
