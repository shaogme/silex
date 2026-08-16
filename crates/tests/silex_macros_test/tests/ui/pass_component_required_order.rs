#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn RequiredOrder<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain] first: String,
    #[chain] second: String,
) -> impl View<'owner> {
    let _ = (owner, first, second);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = RequiredOrder(ctx, AnyView::Empty)
            .second(String::from("second"))
            .first(String::from("first"))
            .second(String::from("override"))
            .build();
        let _ = AnyView::new(view);
    });
}
