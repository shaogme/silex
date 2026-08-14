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
struct ExplicitProps<'scope> {
    #[ctx]
    ctx: SilexContext<'scope>,
    children: AnyView<'scope>,
}

fn render_explicit<'scope>(props: ExplicitProps<'scope>) -> impl View<'scope> {
    let _ = props.ctx;
    props.children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(scope, error_handler);
        let view = Explicit(ctx, AnyView::Empty).build();
        let _ = AnyView::new(view);
    });
}
