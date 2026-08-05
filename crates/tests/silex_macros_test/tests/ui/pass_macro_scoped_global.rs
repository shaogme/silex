#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::Hex;
use silex_core::reactivity::Signal;
use silex_macros::global;

global! {
    pub GlobalTheme<'scope>(
        color: Signal<'scope, Hex>,
        selector: Signal<'scope, String>,
    ) {
        :root { color: $(color); }
        $selector { border-color: $(color); }
    }
}

fn main() {}
