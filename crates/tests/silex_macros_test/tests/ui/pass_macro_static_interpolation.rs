#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_macros::{css, global, styled, theme};
use silex_css::types::Hex;
use silex_dom::attribute::{AttributeBuilder, GlobalAttributes};

theme! {
    pub struct StaticTheme {
        pub primary: Hex,
    }
}

global! {
    pub StaticGlobal<'scope>(scope: silex_core::Scope<'scope>) {
        body { color: $(static StaticTheme::PRIMARY); }
    }
}

global! {
    pub MixedGlobal<'scope>(
        error_handler: silex_core::ErrorReporter<'scope>,
        color: silex_core::reactivity::Signal<'scope, Hex>,
    ) {
        body {
            color: $(static StaticTheme::PRIMARY);
            border-color: $(color);
        }
    }
}

styled! {
    pub StaticStyled<'scope><div>(
        error_handler: silex_core::ErrorReporter<'scope>,
        children: silex_dom::view::AnyView<'scope>,
    ) {
        color: $(static StaticTheme::PRIMARY);
    }
}

styled! {
    pub MixedStyled<'scope><div>(
        error_handler: silex_core::ErrorReporter<'scope>,
        children: silex_dom::view::AnyView<'scope>,
        color: silex_core::reactivity::Signal<'scope, Hex>,
    ) {
        color: $(static StaticTheme::PRIMARY);
        border-color: $(color);
    }
}

fn main() {
    let _ = css! { color: $(static StaticTheme::PRIMARY); };
    let mut runtime = silex_core::Runtime::new();
    let _ = runtime.child(|scope| {
        let _ = StaticGlobal(scope);
    });
}
