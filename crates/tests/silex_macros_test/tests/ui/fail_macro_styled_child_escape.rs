#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_css::types::Hex;
use silex_core::Runtime;
use silex_dom::attribute::{AttributeBuilder, GlobalAttributes};
use silex_dom::prelude::AnyView;
use silex_macros::styled;

styled! {
    pub ScopedPanel<'scope><div>(
        children: AnyView<'scope>,
        color: silex_core::reactivity::Signal<'scope, Hex>,
    ) {
        color: $(color);
    }
}

fn main() {
    let mut runtime = Runtime::new();
    let view = runtime.child(|scope| {
        let (color, _) = scope.signal(silex_css::types::hex("#fff")).unwrap();
        ScopedPanel(AnyView::new(()), color)
    });
    let _ = view;
}
