#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_view::prelude::*;
use silex_macros::component;

#[component]
fn FallibleWithoutQuestion<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(default)] callback: Callback<'owner, String>,
) -> impl View<'owner> {
    let _ = (owner, callback);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let _view: FallibleWithoutQuestionComponent<SilexContext<'_>> = FallibleWithoutQuestion(
            ctx,
            AnyView::Empty,
        )
        .build();
    });
}
