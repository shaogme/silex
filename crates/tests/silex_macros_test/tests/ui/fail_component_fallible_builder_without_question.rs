#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn FallibleWithoutQuestion<'scope>(
    scope: Scope<'scope>,
    children: AnyView<'scope>,
    #[chain(default)] callback: Callback<'scope, String>,
) -> impl View<'scope> {
    let _ = (scope, callback);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _view: FallibleWithoutQuestionComponent =
            FallibleWithoutQuestion(scope, AnyView::Empty).build();
    });
}
