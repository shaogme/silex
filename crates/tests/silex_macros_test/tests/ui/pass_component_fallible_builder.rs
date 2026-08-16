#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn FallibleBuilder<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain] value: String,
    #[chain(default)] callback: Callback<'owner, String>,
) -> impl View<'owner> {
    let _ = (owner, value, callback);
    children
}

fn build_view<'owner>(owner: OwnerAccess<'owner>) -> SilexResult<impl View<'owner>> {
    let callback = owner.callback(|_: String| Ok(()))?;
    let error_handler = owner.error_handler(|_| {})?;
    let ctx = SilexContext::new(owner, error_handler.view());
    Ok(FallibleBuilder(ctx, AnyView::Empty)
        .value(String::from("ready"))
        .callback(callback)
        .build()?)
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let _ = build_view(owner);
    });
}
