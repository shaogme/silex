#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_css::prelude::*;
use silex_macros::theme;

theme! {
    pub struct PatchTheme {
        pub primary: Hex,
    }
}

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| -> SilexResult<()> {
            let error_handler = scope.error_handler(|_| {})?;
            let ctx = SilexContext::new(scope, error_handler);
            let patch = rx!(
                ctx;
                @fn PatchThemePatch::default().primary(hex("#ff69b4"))
            );
            let _ = theme_patch(patch);
            Ok(())
        })
        .unwrap()
        .unwrap();
}
