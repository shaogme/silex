#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::{css, global, styled, theme};
use silex_css::types::Hex;

theme! {
    pub struct StaticTheme {
        pub primary: Hex,
    }
}

global! {
    pub StaticGlobal<'owner>(owner: silex_core::OwnerAccess<'owner>) {
        body { color: $(static StaticTheme::PRIMARY); }
    }
}

global! {
    pub MixedGlobal<'owner>(
        error_handler: silex_core::ErrorReporter<'owner>,
        color: silex_core::reactivity::Rx<'owner, Hex>,
    ) {
        body {
            color: $(static StaticTheme::PRIMARY);
            border-color: $(color);
        }
    }
}

styled! {
    pub StaticStyled<'owner><div>(
        #[ctx] ctx: silex_core::SilexContext<'owner>,
        children: silex_view::AnyView<'owner>,
    ) {
        color: $(static StaticTheme::PRIMARY);
    }
}

styled! {
    pub MixedStyled<'owner><div>(
        #[ctx] ctx: silex_core::SilexContext<'owner>,
        children: silex_view::AnyView<'owner>,
        color: silex_core::reactivity::Rx<'owner, Hex>,
    ) {
        color: $(static StaticTheme::PRIMARY);
        border-color: $(color);
    }
}

fn main() {
    let _ = css! { color: $(static StaticTheme::PRIMARY); };
    let mut runtime = silex_core::Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let _ = StaticGlobal(owner);
    });
}
