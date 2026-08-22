#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn NamedChain<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(name = replace_items)] required_items: Vec<String>,
    #[chain(name = push_item, each)] collected_items: Vec<String>,
    #[chain(name = "set_tags", default)] tags: Vec<String>,
) -> impl View<'owner> {
    let _ = (ctx, required_items, collected_items, tags);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.with_transient(|owner| {
        let error_handler = owner.error_handler(|_| {}).expect("handler");
        let ctx = SilexContext::new(owner, error_handler.view());
        let view = NamedChain(ctx, AnyView::Empty)
            .replace_items(vec![String::from("first"), String::from("second")])
            .push_item("collected")
            .push_item(String::from("another-collected"))
            .set_tags(vec![String::from("tag"), String::from("another-tag")])
            .build();
        let _ = AnyView::new(view);
    });
}
