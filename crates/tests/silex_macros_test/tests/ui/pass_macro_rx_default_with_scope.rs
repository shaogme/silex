#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn RxDefaultWithScope<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[chain(default)] signal: Signal<'scope, i32>,
    #[chain(default)] read: ReadSignal<'scope, i32>,
    #[chain(default)] rw: RwSignal<'scope, i32>,
    #[chain(default)] memo: Memo<'scope, i32>,
    #[chain(default)] stored: StoredValue<'scope, i32>,
    #[chain(default)] rx: Rx<'scope, i32>,
    #[chain(default)] callback: Callback<'scope, ()>,
    #[chain(default)] node_ref: NodeRef<'scope, String>,
) -> impl View<'scope> {
    let _ = (
        scope, signal, read, rw, memo, stored, rx, callback, node_ref,
    );
    children
}

#[component]
fn RxDefaultExplicit<'scope, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'scope>,
    #[prop(into)]
    #[chain(default = "column")]
    direction: Signal<'scope, String>,
) -> impl View<'scope> {
    let _ = direction;
    let _ = scope;
    children
}

#[component]
fn OptionalRxDefault<'scope, Ctx>(
    children: AnyView<'scope>,
    #[ctx] ctx: Ctx,
    #[chain(default)] value: Option<Signal<'scope, i32>>,
) -> impl View<'scope> {
    let _ = value;
    children
}

fn main() {}
