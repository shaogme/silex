use silex_core::{ErrorHandler, ErrorReporter, Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler: ErrorHandler<'_, SilexError> = scope.error_handler(|error| {
            let _ = error;
        });
        let reporter: ErrorReporter<'_> = handler;
        reporter.handle(SilexError::Framework("compile-pass".to_string()));
    });
}
