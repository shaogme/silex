#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::reactivity::Signal;
use silex_css::types::Hex;
use silex_macros::global;

global! {
    pub StaticValueGlobal<'scope>(source: Signal<'scope, Hex>) {
        body {
            color: $("red");
        }
    }
}

fn main() {}
