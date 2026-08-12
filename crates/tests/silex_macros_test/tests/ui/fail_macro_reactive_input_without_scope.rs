#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn NoReactiveInputScope<'scope>(
    #[chain] source: Signal<'scope, bool>,
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    source
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let source: Signal<'_, bool> = true.into_reactive_input(scope);
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let _ = NoReactiveInputScope()
            .error_handler(error_handler)
            .source(source)
            .source(true);
    });
}
