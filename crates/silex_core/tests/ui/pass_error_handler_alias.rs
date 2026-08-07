use silex_core::{ErrorHandler, ErrorReporter, SilexError};

fn main() {
    let handler: ErrorHandler<'_, SilexError> = ErrorHandler::new(|error| {
        let _ = error;
    });
    let reporter: ErrorReporter<'_> = handler.clone();
    reporter.handle(SilexError::Framework("compile-pass".to_string()));
}
