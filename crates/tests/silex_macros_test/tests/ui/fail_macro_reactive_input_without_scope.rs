#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn NoReactiveInputScope<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    #[chain] source: Rx<'owner, bool>,
) -> impl View<'owner> {
    source
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let source: Rx<'_, bool> = true.into_reactive_input(owner);
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let _ = NoReactiveInputScope(ctx)
            .source(source)
            .source(true);
    });
}
