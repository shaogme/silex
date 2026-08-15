#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuildAttributes<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
) -> impl View<'scope> {
    let _ = scope;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(scope, error_handler.view());
        let mut view = BuildAttributes(ctx, AnyView::Empty)
            .class("before")
            .build()
            .class("after");
        view.apply_attributes(Vec::new());
        let _ = AnyView::new(view);
    });
}
