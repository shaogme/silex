#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_macros::css;

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| -> SilexResult<()> {
        let error_handler = owner.error_handler(|_| {})?;
        let color = "red";
        let _ = css!(error_handler; color: $color;)?;
        Ok(())
    });
}
