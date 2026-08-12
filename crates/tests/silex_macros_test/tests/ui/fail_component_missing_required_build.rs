#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn MissingRequiredBuild<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
    #[chain] required: String,
) -> impl View<'scope> {
    let _ = (scope, required);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let _ = MissingRequiredBuild(scope, AnyView::Empty)
            .error_handler(error_handler)
            .build();
    });
}
