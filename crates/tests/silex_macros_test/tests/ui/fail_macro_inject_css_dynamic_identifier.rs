#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::inject_css;

fn main() {
    inject_css! { color: $color; };
}
