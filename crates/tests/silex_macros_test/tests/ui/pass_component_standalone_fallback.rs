#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::PropsBuilder;

#[derive(Clone, PropsBuilder)]
struct StandaloneProps<'owner> {
    #[ctx]
    ctx: SilexContext<'owner>,
    children: AnyView<'owner>,
}

#[allow(non_snake_case)]
fn __silex_render_Standalone<'owner>(props: StandaloneProps<'owner>) -> impl View<'owner> {
    let _ = props.ctx;
    props.children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = Standalone(ctx, AnyView::Empty).build();
        let _ = AnyView::new(view);
    });
}
