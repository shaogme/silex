#![allow(unused_extern_crates)]
#![allow(dead_code)]

include!("../../src/lib.rs");

use silex_macros::css;

#[derive(Default)]
struct Legacy;

trait IntoRx {}

impl IntoRx for Legacy {}

fn main() {
    let _ = css! { color: $(Legacy::default()); };
}
