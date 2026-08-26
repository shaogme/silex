#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::component;

#[component]
fn MissingRxDefaultScope<'owner>(
    children: silex_view::elements::AnyView<'owner>,
    #[chain(default)] value: silex_core::reactivity::Rx<'owner, i32>,
) -> impl silex_view::mount::View<'owner> {
    let _ = value;
    children
}

fn main() {}
