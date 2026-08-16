#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuilderAsView<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
) -> impl View<'owner> {
    let _ = owner;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let _ = AnyView::new(
            BuilderAsView(ctx, AnyView::Empty),
        );
    });
}
