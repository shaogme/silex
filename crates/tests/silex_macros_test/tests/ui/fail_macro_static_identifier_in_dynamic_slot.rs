#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_macros::css;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| -> SilexResult<()> {
        let error_handler = scope.error_handler(|_| {})?;
        let color = "red";
        let _ = css!(error_handler; color: $color;)?;
        Ok(())
    });
}
