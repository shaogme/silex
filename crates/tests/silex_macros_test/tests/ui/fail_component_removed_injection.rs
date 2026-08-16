#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::{ErrorReporter, OwnerAccess};
use silex_macros::component;

#[component]
fn RemovedInjection<'owner, Ctx>(
#[ctx] ctx: Ctx,
    #[inject(owner)] owner: (),
    
) -> impl View<'owner> {
    let _ = (owner, owner, error_handler);
    ()
}

fn main() {}
