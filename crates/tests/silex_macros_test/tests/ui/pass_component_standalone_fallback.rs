#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::PropsBuilder;

#[derive(Clone, PropsBuilder)]
struct StandaloneProps<'scope> {
    #[context]
    context: SilexContext<'scope>,
    children: AnyView<'scope>,
}

#[allow(non_snake_case)]
fn __silex_render_Standalone<'scope>(props: StandaloneProps<'scope>) -> impl View<'scope> {
    let _ = props.context;
    props.children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let context = SilexContext::new(scope, error_handler);
        let view = Standalone(context, AnyView::Empty).build();
        let _ = AnyView::new(view);
    });
}
