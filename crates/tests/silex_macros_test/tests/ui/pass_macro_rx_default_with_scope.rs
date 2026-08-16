#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn RxDefaultWithScope<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(default)] signal: Signal<'owner, i32>,
    #[chain(default)] read: ReadSignal<'owner, i32>,
    #[chain(default)] rw: RwSignal<'owner, i32>,
    #[chain(default)] computed: Computed<'owner, i32>,
    #[chain(default)] stored: StoredValue<'owner, i32>,
    #[chain(default)] rx: Rx<'owner, i32>,
    #[chain(default)] callback: Callback<'owner, ()>,
    #[chain(default)] node_ref: NodeRef<'owner, String>,
) -> impl View<'owner> {
    let _ = (
        owner, signal, read, rw, computed, stored, rx, callback, node_ref,
    );
    children
}

#[component]
fn RxDefaultExplicit<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[prop(into)]
    #[chain(default = "column")]
    direction: Signal<'owner, String>,
) -> impl View<'owner> {
    let _ = direction;
    let _ = owner;
    children
}

#[component]
fn OptionalRxDefault<'owner, Ctx>(
    children: AnyView<'owner>,
    #[ctx] ctx: Ctx,
    #[chain(default)] value: Option<Signal<'owner, i32>>,
) -> impl View<'owner> {
    let _ = value;
    children
}

fn main() {}
