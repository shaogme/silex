use silex_core::{ErrorReporter, Runtime};
use silex_dom::view::MountOwnerToken;

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let token = owner.error_handler(|_| {}).expect("handler");
            let error_handler: ErrorReporter<'_> = token.view();
            let _token = MountOwnerToken::new(owner);
            let _ = error_handler;
        })
        .expect("transient owner should initialize");
}
