use silex_core::{ErrorReporter, Runtime, SilexError};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let reporter: ErrorReporter<'_> = scope.error_handler(|_: SilexError| {});
        reporter.report(SilexError::Framework(String::from("removed")));
    });
}
