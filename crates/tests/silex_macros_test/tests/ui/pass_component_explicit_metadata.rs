#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::PropsBuilder;

#[derive(Clone, PropsBuilder)]
#[silex_component(
    builder = ExplicitBuilder,
    product = ExplicitProduct,
    render = render_explicit,
)]
struct ExplicitProps<'owner> {
    #[ctx]
    ctx: SilexContext<'owner>,
    children: AnyView<'owner>,
}

fn render_explicit<'owner>(props: ExplicitProps<'owner>) -> impl View<'owner> {
    let _ = props.ctx;
    props.children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = Explicit(ctx, AnyView::Empty).build();
        let _ = AnyView::new(view);
    });
}
