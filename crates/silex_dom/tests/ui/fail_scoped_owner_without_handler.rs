use silex_core::Runtime;
use silex_dom::view::ScopedViewOwner;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let _owner = ScopedViewOwner::new(scope);
    });
}
