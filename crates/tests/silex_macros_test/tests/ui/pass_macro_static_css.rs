#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::{css, global, inject_css, tw};

global! {
    body { color: red; }
}

fn main() {
    let _ = css! { color: red; };
    let _ = tw!("inline-flex items-center");
    inject_css! { :root { --test-color: red; } };
}
