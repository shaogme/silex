#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::Hex;
use silex_view::AnyView;
use silex_macros::styled;

styled! {
    pub ScopedPanel<'owner><div>(
        #[ctx] ctx: silex_core::SilexContext<'owner>,
        children: AnyView<'owner>,
        color: silex_core::reactivity::Rx<'owner, Hex>,
    ) {
        color: $(color);
    }
}

fn main() {}
