#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn FallibleBuilder<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain] value: String,
    #[chain(default)] callback: Callback<'scope, String>,
) -> impl View<'scope> {
    let _ = (scope, value, callback);
    children
}

fn build_view<'scope>(scope: Scope<'scope>) -> SilexResult<impl View<'scope>> {
    let callback = scope.callback(|_: String| Ok(()))?;
    let error_handler = scope.error_handler(|_| {})?;
    let ctx = SilexContext::new(scope, error_handler.view());
    Ok(FallibleBuilder(ctx, AnyView::Empty)
        .value(String::from("ready"))
        .callback(callback)
        .build()?)
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let _ = build_view(scope);
    });
}
