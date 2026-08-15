use silex_core::{ErrorHandlerToken, ErrorReporter, Runtime, SilexError, SilexErrorKind};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let token: ErrorHandlerToken<'_> = scope
            .error_handler(|_: SilexError| {})
            .expect("handler should register");
        let reporter: ErrorReporter<'_> = token.view();
        reporter.report(SilexError::recoverable(SilexErrorKind::Framework(String::from("removed"))));
    });
}
