#![deny(warnings)]

include!("../src/lib.rs");

use silex_core::prelude::*;
use silex_macros::component;
use silex_view::prelude::*;

#[component]
fn ChainCollection<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[chain(name = set_items)] items: Vec<String>,
    #[chain(name = add_item, each)] collected: Vec<String>,
) -> impl View<'owner> {
    let _ = (ctx, items, collected);
    children
}

#[test]
fn vec_chain_modes_keep_their_declared_semantics() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let error_handler = owner.error_handler(|_| {})?;
            let ctx = SilexContext::new(owner, error_handler.view());
            let product = ChainCollection(ctx, AnyView::Empty)
                .set_items(vec![String::from("replaced")])
                .add_item("first")
                .add_item(String::from("second"))
                .build();

            assert_eq!(product.props.items, vec!["replaced"]);
            assert_eq!(product.props.collected, vec!["first", "second"]);
            Ok::<_, SilexError>(())
        })
        .expect("transient owner should be available")
        .expect("builder should construct both Vec modes");
}
