#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuildAttributes<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let _ = scope;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let mut view = BuildAttributes(scope, AnyView::Empty)
            .error_handler(error_handler)
            .class("before")
            .build()
            .class("after");
        view.apply_attributes(Vec::new());
        let _ = AnyView::new(view);
    });
}
