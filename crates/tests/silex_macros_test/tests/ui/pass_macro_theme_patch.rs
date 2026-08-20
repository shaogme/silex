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
        .with_transient(|owner| -> SilexResult<()> {
            let error_handler = owner.error_handler(|_| {})?;
            let ctx = SilexContext::new(owner, error_handler.view());
            let patch = rx!(
                ctx;
                @fn PatchThemePatch::default().primary(hex("#ff69b4"))
            )?;
            let _ = theme_patch(patch);
            Ok(())
        })
        .unwrap()
        .unwrap();
}
