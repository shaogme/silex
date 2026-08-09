#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;
use std::marker::PhantomData;

#[component]
fn RawPropsAsView<'scope>(scope: Scope<'scope>, children: AnyView<'scope>) -> impl View<'scope> {
    let _ = scope;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let props = RawPropsAsViewProps {
            scope,
            children: AnyView::Empty,
            __silex_scope_marker: PhantomData,
        };
        let _ = AnyView::new(props);
    });
}
