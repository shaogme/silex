#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::{ErrorReporter, Scope};
use silex_macros::component;

#[component]
fn RemovedInjection<'scope>(
    scope: Scope<'scope>,
    #[inject(owner)] owner: (),
    #[chain] error_handler: ErrorReporter<'scope>,
) -> impl View<'scope> {
    let _ = (scope, owner, error_handler);
    ()
}

fn main() {}
