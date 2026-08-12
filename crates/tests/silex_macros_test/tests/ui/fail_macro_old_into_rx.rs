#![allow(unused_extern_crates)]
#![allow(dead_code)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_macros::css;

#[derive(Default)]
struct Legacy;

trait IntoRx {}

impl IntoRx for Legacy {}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| -> SilexResult<()> {
        let error_handler = scope.error_handler(|_| {})?;
        let _ = css!(error_handler; color: $(Legacy::default());)?;
        Ok(())
    });
}
