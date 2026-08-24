#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_view::prelude::*;
use silex_macros::component;

#[component]
fn ConflictingAttributes<'owner, Ctx>(
    #[ctx] ctx: Ctx,
    children: AnyView<'owner>,
    #[attrs] #[chain(each)] attrs: Vec<AttributeGroup<'owner>>,
) -> impl View<'owner> {
    let _ = (ctx, attrs);
    children
}

fn main() {}
