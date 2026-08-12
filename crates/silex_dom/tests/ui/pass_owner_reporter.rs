use silex_core::{ErrorReporter, Runtime, SilexError};
use silex_dom::view::{ScopedViewOwner, ViewOwner};

fn main() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let error_handler: ErrorReporter<'_> = scope.error_handler(|_| {}).expect("handler");
            let owner = ScopedViewOwner::new(scope, error_handler);
            let token = owner.token();
            token.handle_error(SilexError::Framework("owner".to_string()));
            let nested = token.with_error_handler(scope.error_handler(|_| {}).expect("handler"));
            nested.handle_error(SilexError::Framework("nested".to_string()));
        })
        .expect("child scope should initialize");
}
