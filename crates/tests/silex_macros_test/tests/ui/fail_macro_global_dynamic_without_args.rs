#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::global;

global! {
    pub DynamicGlobal<'owner>() {
        body {
            color: $(color);
        }
    }
}

fn main() {}
