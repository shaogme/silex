#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn MissingRequiredBuild<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain] required: String,
) -> impl View<'scope> {
    let _ = (scope, required);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(scope, error_handler);
        let _ = MissingRequiredBuild(ctx, AnyView::Empty)
            .build();
    });
}
