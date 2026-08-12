#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::Hex;
use silex_dom::attribute::{AttributeBuilder, GlobalAttributes};
use silex_dom::prelude::AnyView;
use silex_macros::styled;

styled! {
    pub ScopedPanel<'scope><div>(
        error_handler: silex_core::ErrorReporter<'scope>,
        children: AnyView<'scope>,
        color: silex_core::reactivity::Signal<'scope, Hex>,
    ) {
        color: $(color);
    }
}

fn main() {}
