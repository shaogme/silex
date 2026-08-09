#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::{css_var, px};
use silex_macros::css;

fn main() {
    let width = px(4);
    let color = css_var("--brand-color");
    let _ = css! {
        width: $width;
        color: $color;
    };
}
