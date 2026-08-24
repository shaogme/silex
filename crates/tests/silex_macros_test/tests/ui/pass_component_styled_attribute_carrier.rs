#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_view::prelude::*;
use silex_html::{AnchorAttributes, FormAttributes, MediaAttributes};
use silex_macros::{component, styled};

styled! {
    pub BridgeButton<'owner, Ctx><button>(
        #[ctx] ctx: Ctx,
        children: AnyView<'owner>,
        #[chain] #[prop(into)] label: String,
        #[chain] #[prop(into)] name: String,
    ) {
        color: red;
    }
}

styled! {
    pub BridgeTextarea<'owner, Ctx><textarea>(
        #[ctx] ctx: Ctx,
    ) {
        color: blue;
    }
}

styled! {
    pub BridgeAnchor<'owner, Ctx><a>(
        #[ctx] ctx: Ctx,
        children: AnyView<'owner>,
    ) {
        color: green;
    }
}

styled! {
    pub BridgeImage<'owner, Ctx><img>(
        #[ctx] ctx: Ctx,
    ) {
        display: block;
    }
}

#[component]
fn UntypedComponent<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
) -> impl View<'owner> {
    let _ = ctx;
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());

        let button = BridgeButton(ctx, AnyView::Empty)
            .type_("button")
            .disabled(true)
            .name("initial")
            .label("button")
            .name("final")
            .build();
        let textarea = BridgeTextarea(ctx).value("text").placeholder("hint").build();
        let anchor = BridgeAnchor(ctx, AnyView::Empty).href("/docs").build();
        let image = BridgeImage(ctx).src("/logo.svg").alt("logo").build();

        let _ = AnyView::new(button);
        let _ = AnyView::new(textarea);
        let _ = AnyView::new(anchor);
        let _ = AnyView::new(image);

        let _ = UntypedComponent(ctx, AnyView::Empty).build();
    });
}
