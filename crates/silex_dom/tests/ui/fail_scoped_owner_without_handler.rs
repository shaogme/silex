use silex_core::Runtime;
use silex_dom::view::ViewOwner;
use silex_dom::view::ScopedViewOwner;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let owner = ScopedViewOwner::new(scope);
        let _ = owner.token().error_handler();
    });
}
