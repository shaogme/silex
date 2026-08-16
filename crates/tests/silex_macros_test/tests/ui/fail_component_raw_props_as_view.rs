#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn RawPropsAsView<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
) -> impl View<'owner> {
    let _ = owner;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let props = RawPropsAsViewProps {
            ctx: SilexContext::new(owner, error_handler.view()),
            children: AnyView::Empty,
        };
        let _ = AnyView::new(props);
    });
}
