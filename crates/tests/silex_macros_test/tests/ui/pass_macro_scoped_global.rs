#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::Hex;
use silex_core::{ErrorReporter, reactivity::Rx};
use silex_macros::global;

global! {
    pub GlobalTheme<'owner>(
        error_handler: ErrorReporter<'owner>,
        color: Rx<'owner, Hex>,
        selector: Rx<'owner, String>,
    ) {
        :root { color: $(color); }
        $selector { border-color: $(color); }
    }
}

fn main() {}
