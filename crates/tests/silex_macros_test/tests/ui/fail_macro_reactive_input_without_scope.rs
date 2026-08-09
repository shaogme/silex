#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn NoReactiveInputScope<'scope>(
    #[chain] source: Signal<'scope, bool>,
) -> impl View<'scope> {
    source
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let source: Signal<'_, bool> = true.into_reactive_input(scope);
        let _ = NoReactiveInputScope().source(source).source(true);
    });
}
