#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::Hex;
use silex_core::{ErrorReporter, reactivity::Signal};
use silex_macros::global;

global! {
    pub GlobalTheme<'owner>(
        error_handler: ErrorReporter<'owner>,
        color: Signal<'owner, Hex>,
        selector: Signal<'owner, String>,
    ) {
        :root { color: $(color); }
        $selector { border-color: $(color); }
    }
}

fn main() {}
