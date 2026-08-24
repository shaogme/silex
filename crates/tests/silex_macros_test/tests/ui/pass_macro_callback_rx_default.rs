#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_view::prelude::*;
use silex_macros::component;

#[component]
fn CallbackRxDefault<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(default)] callback: Callback<'owner, String>,
) -> impl View<'owner> {
    let _ = (owner, callback);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let _view = CallbackRxDefault(ctx, AnyView::Empty)
            .build()
            .expect("callback default should be fallible");
    });
}
