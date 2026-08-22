#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::{ErrorHandlerToken, Runtime, SilexError, SilexResult};
use silex_macros::{css, global, tw};

global! {
    pub TokenGlobal<'owner>(
        error_handler: ErrorHandlerToken<'owner>,
        color: silex_core::reactivity::Rx<'owner, silex_css::types::Hex>,
    ) {
        body {
            color: $(color);
            border-color: $(color);
        }
    }
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| -> SilexResult<()> {
        let width = owner.signal(silex_css::types::px(4))?;
        let color = owner.signal(silex_css::types::hex("#123456"))?;
        let token = owner.error_handler(|_: SilexError| {})?;

        let _css: silex_core::SilexResult<silex_css::DynamicCss<'_>> =
            css!(token.clone(); width: $(width); height: $(width););
        let _tw: silex_core::SilexResult<silex_css::DynamicCss<'_>> =
            tw!(token.clone(); "w-[$(width)]");
        let _global = TokenGlobal(token, color.into_rx())?;
        Ok(())
    });
}
