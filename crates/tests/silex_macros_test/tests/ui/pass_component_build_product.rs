#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn BuiltProduct<'scope, Ctx>(
#[context] context: Ctx,
    children: AnyView<'scope>,
    
    #[chain] label: String,
) -> impl View<'scope> {
    let _ = (scope, label);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let context = SilexContext::new(scope, error_handler);
        let view = BuiltProduct(context, AnyView::Empty)
            .label(String::from("Save"))
            .build();
        let _ = AnyView::new(view);
    });
}
