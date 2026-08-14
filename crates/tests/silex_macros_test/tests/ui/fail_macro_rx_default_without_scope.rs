#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn MissingRxDefaultScope<'scope>(
    children: AnyView<'scope>,
    #[chain(default)] value: Signal<'scope, i32>,
) -> impl View<'scope> {
    let _ = value;
    children
}

fn main() {}
