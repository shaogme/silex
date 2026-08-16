#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::Hex;
use silex_core::Runtime;
use silex_dom::attribute::{AttributeBuilder, GlobalAttributes};
use silex_dom::prelude::AnyView;
use silex_macros::styled;

styled! {
    pub ScopedPanel<'owner><div>(
        #[ctx] ctx: silex_core::SilexContext<'owner>,
        children: AnyView<'owner>,
        color: silex_core::reactivity::Signal<'owner, Hex>,
    ) {
        color: $(color);
    }
}

fn main() {
    let mut runtime = Runtime::new();
    let view = runtime.with_transient(|owner| {
        let (color, _) = owner.signal(silex_css::types::hex("#fff")).unwrap();
        let error_handler = owner.error_handler(|_| {}).unwrap();
        let ctx = silex_core::SilexContext::new(owner, error_handler.view());
        ScopedPanel(ctx, color, color)
    });
    let _ = view;
}
