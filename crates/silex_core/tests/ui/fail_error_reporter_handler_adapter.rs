use silex_core::{ErrorReporter, Runtime, SilexError, ErrorHandlerToken};

fn main() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let token: ErrorHandlerToken<'_> = scope
            .error_handler(|_: SilexError| {})
            .expect("handler should register");
        let reporter: ErrorReporter<'_> = token.view();
        let _ = reporter.handler();
    });
}
