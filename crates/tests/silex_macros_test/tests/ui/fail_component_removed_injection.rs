#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::{ErrorReporter, Scope};
use silex_macros::component;

#[component]
fn RemovedInjection<'scope, Ctx>(
#[context] context: Ctx,
    #[inject(owner)] owner: (),
    
) -> impl View<'scope> {
    let _ = (scope, owner, error_handler);
    ()
}

fn main() {}
