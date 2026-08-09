#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::css;

fn main() {
    let color = "red";
    let _ = css! { color: $color; };
}
