#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::global;

global! {
    body {
        color: red;
    }
}

fn main() {}
