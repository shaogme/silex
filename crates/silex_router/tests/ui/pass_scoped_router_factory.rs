use std::rc::Rc;

use silex_core::Runtime;
use silex_router::dom::view::{AnyView, ScopeView};
use silex_router::RouterViewFactory;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, _) = scope.signal(String::from("scoped"));
        let factory = RouterViewFactory(Rc::new(move || {
            AnyView::new(ScopeView::new(value))
        }));
        let _ = factory;
    });
}
