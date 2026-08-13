use silex_core::{ErrorHandler, ErrorReporter, Runtime, SilexError, SilexErrorKind};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler: ErrorHandler<'_, SilexError> = scope.error_handler(|error| {
            let _ = error;
        }).expect("handler should register");
        let reporter: ErrorReporter<'_> = handler;
        let _ = reporter.handle(SilexError::recoverable(SilexErrorKind::Framework("compile-pass".to_string())));
    }).expect("child scope should initialize");
}
