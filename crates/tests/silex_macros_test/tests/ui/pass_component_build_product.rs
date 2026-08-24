#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_view::prelude::*;
use silex_macros::component;

#[component]
fn BuiltProduct<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain] label: String,
) -> impl View<'owner> {
    let _ = (owner, label);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = BuiltProduct(ctx, AnyView::Empty)
            .label(String::from("Save"))
            .build();
        let _ = AnyView::new(view);
    });
}
