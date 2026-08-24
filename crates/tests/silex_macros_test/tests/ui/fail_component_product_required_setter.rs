#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_view::prelude::*;
use silex_macros::component;

#[component]
fn ProductRequiredSetter<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain] mandatory: String,
) -> impl View<'owner> {
    let _ = (owner, mandatory);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let product = ProductRequiredSetter(ctx, AnyView::Empty)
            .mandatory(String::from("value"))
            .build();
        let _ = product.mandatory(String::from("replacement"));
    });
}
