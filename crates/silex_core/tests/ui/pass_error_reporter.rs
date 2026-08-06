use silex_core::{ErrorReporter, SilexError};

fn main() {
    let reporter = ErrorReporter::new(|error| {
        let _ = error;
    });
    reporter.report(SilexError::Framework("compile-pass".to_string()));
}
