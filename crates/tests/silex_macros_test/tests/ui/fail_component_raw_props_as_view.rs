#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;
use std::marker::PhantomData;

#[component]
fn RawPropsAsView<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
) -> impl View<'scope> {
    let _ = scope;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let props = RawPropsAsViewProps {
            ctx: SilexContext::new(
                scope,
                scope.error_handler(|_| {}).expect("handler"),
            ),
            children: AnyView::Empty,
            __silex_scope_marker: PhantomData,
        };
        let _ = AnyView::new(props);
    });
}
