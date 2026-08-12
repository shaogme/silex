use silex_core::{ErrorReporter, Runtime};
use silex_dom::view::{ScopedViewOwner, ViewOwner};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let error_handler: ErrorReporter<'_> = scope.error_handler(|_| {}).expect("handler");
            let _owner = ScopedViewOwner::new(scope);
            let _token = _owner.token();
            let _ = error_handler;
        })
        .expect("child scope should initialize");
}
