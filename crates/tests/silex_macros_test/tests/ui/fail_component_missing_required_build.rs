#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn MissingRequiredBuild<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain] required: String,
) -> impl View<'owner> {
    let _ = (owner, required);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let _ = MissingRequiredBuild(ctx, AnyView::Empty)
            .build();
    });
}
