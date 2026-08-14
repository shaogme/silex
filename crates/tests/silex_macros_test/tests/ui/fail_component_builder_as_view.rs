#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuilderAsView<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
) -> impl View<'scope> {
    let _ = scope;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(scope, error_handler);
        let _ = AnyView::new(
            BuilderAsView(ctx, AnyView::Empty),
        );
    });
}
