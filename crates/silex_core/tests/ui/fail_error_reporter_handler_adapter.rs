use silex_core::{ErrorHandler, ErrorReporter, SilexError};

fn main() {
    let reporter: ErrorReporter<'_> = ErrorHandler::new(|_: SilexError| {});
    let _ = reporter.handler();
}
