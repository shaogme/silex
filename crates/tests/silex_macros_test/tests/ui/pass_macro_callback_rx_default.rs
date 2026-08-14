#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn CallbackRxDefault<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain(default)] callback: Callback<'scope, String>,
) -> impl View<'scope> {
    let _ = (scope, callback);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(scope, error_handler);
        let _view = CallbackRxDefault(ctx, AnyView::Empty)
            .build()
            .expect("callback default should be fallible");
    });
}
