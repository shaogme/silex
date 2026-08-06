use silex_core::{ErrorReporter, Runtime, SilexError};
use silex_dom::view::{ScopedViewOwner, ViewOwner};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let reporter = ErrorReporter::new(|_| {});
        let owner = ScopedViewOwner::with_error_reporter(scope, reporter);
        let token = owner.token();
        token.report_error(SilexError::Framework("owner".to_string()));
        let nested = token.with_error_reporter(ErrorReporter::new(|_| {}));
        nested.report_error(SilexError::Framework("nested".to_string()));
    });
}
