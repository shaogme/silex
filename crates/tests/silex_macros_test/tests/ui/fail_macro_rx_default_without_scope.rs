#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn MissingRxDefaultScope<'owner>(
    children: AnyView<'owner>,
    #[chain(default)] value: Rx<'owner, i32>,
) -> impl View<'owner> {
    let _ = value;
    children
}

fn main() {}
