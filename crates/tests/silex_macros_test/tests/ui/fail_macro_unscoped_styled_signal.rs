#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::styled;

styled! {
    pub UnscopedPanel<div>(
        children: silex_view::AnyView,
        color: silex_core::reactivity::Rx<'owner, String>,
    ) {
        color: $(color);
    }
}

fn main() {}
