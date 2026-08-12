#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_css::types::{css_var, px};
use silex_macros::css;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| -> SilexResult<()> {
        let error_handler = scope.error_handler(|_| {})?;
        let width = px(4);
        let color = css_var("--brand-color");
        let _ = css!(error_handler; {
            width: $width;
            color: $color;
        })?;
        Ok(())
    });
}
