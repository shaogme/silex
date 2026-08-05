#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::styled;

styled! {
    pub UnscopedPanel<div>(
        children: silex_dom::prelude::AnyView,
        color: silex_core::reactivity::Signal<String>,
    ) {
        color: $(color);
    }
}

fn main() {}
