#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::{ErrorReporter, reactivity::Rx};
use silex_css::types::Hex;
use silex_macros::global;

global! {
    pub StaticValueGlobal<'owner>(
        error_handler: ErrorReporter<'owner>,
        source: Rx<'owner, Hex>,
    ) {
        body {
            color: $("red");
        }
    }
}

fn main() {}
