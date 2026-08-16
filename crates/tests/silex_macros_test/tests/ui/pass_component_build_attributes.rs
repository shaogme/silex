#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuildAttributes<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
) -> impl View<'owner> {
    let _ = owner;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let mut view = BuildAttributes(ctx, AnyView::Empty)
            .class("before")
            .build()
            .class("after");
        view.apply_attributes(Vec::new());
        let _ = AnyView::new(view);
    });
}
