#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::PropsBuilder;

#[derive(Clone, PropsBuilder)]
struct StandaloneProps<'scope> {
    scope: Scope<'scope>,
    children: AnyView<'scope>,
}

#[allow(non_snake_case)]
fn __silex_render_Standalone<'scope>(props: StandaloneProps<'scope>) -> impl View<'scope> {
    let _ = props.scope;
    props.children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let view = Standalone(scope, AnyView::Empty).build();
        let _ = AnyView::new(view);
    });
}
