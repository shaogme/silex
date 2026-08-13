use silex_core::Runtime;
use silex_dom::view::MountOwner;
use silex_dom::view::ScopedMountOwner;

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let owner = ScopedMountOwner::new(scope);
        let _ = owner.token().error_handler();
    });
}
