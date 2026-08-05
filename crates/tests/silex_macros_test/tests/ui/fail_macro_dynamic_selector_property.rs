#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::css;

fn main() {
    let _ = css! { $selector { colr: red; } };
}
