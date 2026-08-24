#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_view::prelude::*;
use silex_html::div;
use silex_macros::component;

#[component]
fn BuildAttributes<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[attrs] attrs: AttributeGroup<'owner>,
) -> impl View<'owner> {
    let _ = owner;
    div(children).apply(attrs)
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = BuildAttributes(ctx, AnyView::Empty)
            .attrs(silex_view::group![("data-kind", "panel")])
            .attr("data-state", "ready")
            .class("before")
            .class("after")
            .on_click(|_| Ok(()))
            .build();
        let _ = AnyView::new(view);
    });
}
