#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::inject_css;

fn main() {
    inject_css! {
        @media (min-width: $width) {
            color: red;
        }
    };
}
