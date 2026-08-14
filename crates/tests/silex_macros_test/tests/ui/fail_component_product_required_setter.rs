#![allow(unused_extern_crates)]

include!("../../src/lib.rs");

use silex_core::prelude::*;
use silex_dom::prelude::*;
use silex_macros::component;

#[component]
fn ProductRequiredSetter<'scope, Ctx>(
#[context] context: Ctx,
    children: AnyView<'scope>,
    
    #[chain] mandatory: String,
) -> impl View<'scope> {
    let _ = (scope, mandatory);
    children
}

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let error_handler = scope.error_handler(|_| {}).expect("handler");
        let context = SilexContext::new(scope, error_handler);
        let product = ProductRequiredSetter(context, AnyView::Empty)
            .mandatory(String::from("value"))
            .build();
        let _ = product.mandatory(String::from("replacement"));
    });
}
