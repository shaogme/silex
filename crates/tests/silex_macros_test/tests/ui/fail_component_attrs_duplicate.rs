#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn DuplicateAttributes<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[attrs] first: AttributeGroup<'owner>,
    #[attrs] second: AttributeGroup<'owner>,
) -> impl View<'owner> {
    let _ = (ctx, first, second);
    children
}

fn main() {}
