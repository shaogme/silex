#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::PropsBuilder;

#[derive(Clone, PropsBuilder)]
struct StandaloneProps<'scope> {
    #[ctx]
    ctx: SilexContext<'scope>,
    children: AnyView<'scope>,
}

#[allow(non_snake_case)]
fn __silex_render_Standalone<'scope>(props: StandaloneProps<'scope>) -> impl View<'scope> {
    let _ = props.ctx;
    props.children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(scope, error_handler);
        let view = Standalone(ctx, AnyView::Empty).build();
        let _ = AnyView::new(view);
    });
}
