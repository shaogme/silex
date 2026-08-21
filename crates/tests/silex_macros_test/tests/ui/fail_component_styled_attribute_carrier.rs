#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::{component, styled};

styled! {
    pub InvalidDiv<'owner, Ctx><div>(
        #[ctx] ctx: Ctx,
        children: AnyView<'owner>,
    ) {
        color: red;
    }
}

styled! {
    pub InvalidSpan<'owner, Ctx><span>(
        #[ctx] ctx: Ctx,
        children: AnyView<'owner>,
    ) {
        color: blue;
    }
}

#[component]
fn UntypedComponent<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
) -> impl View<'owner> {
    let _ = ctx;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());

        let _ = InvalidDiv(ctx, AnyView::Empty).value("invalid");
        let _ = InvalidSpan(ctx, AnyView::Empty).href("invalid");
        let _ = UntypedComponent(ctx, AnyView::Empty).value("invalid");
    });
}
