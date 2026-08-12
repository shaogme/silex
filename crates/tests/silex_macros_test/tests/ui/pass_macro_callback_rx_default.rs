#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn CallbackRxDefault<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain] error_handler: ErrorReporter<'scope>,
    #[chain(default)] callback: Callback<'scope, String>,
) -> impl View<'scope> {
    let _ = (scope, callback);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let _view = CallbackRxDefault(scope, AnyView::Empty)
            .error_handler(error_handler)
            .build()
            .expect("callback default should be fallible");
    });
}
