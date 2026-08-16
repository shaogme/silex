#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::reactivity::Signal;
use silex_css::types::Hex;
use silex_dom::attribute::{AttributeBuilder, GlobalAttributes};
use silex_dom::prelude::AnyView;
use silex_macros::styled;

styled! {
    pub StaticValuePanel<'owner><div>(
        #[ctx] ctx: silex_core::SilexContext<'owner>,
        children: AnyView<'owner>,
        source: Signal<'owner, Hex>,
    ) {
        color: $("red");
    }
}

fn main() {}
