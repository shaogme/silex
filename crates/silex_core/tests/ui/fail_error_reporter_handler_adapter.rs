use silex_core::{ErrorReporter, Runtime, SilexError, ErrorHandlerToken};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let token: ErrorHandlerToken<'_> = owner
            .error_handler(|_: SilexError| {})
            .expect("handler should register");
        let reporter: ErrorReporter<'_> = token.view();
        let _ = reporter.handler();
    });
}
