use silex_core::{
    ErrorHandler, ErrorHandlerToken, ErrorReporter, Runtime, SilexError, SilexErrorKind,
};

fn main() {
    let mut runtime = Runtime::new();
    runtime.with_transient(|owner| {
        let token: ErrorHandlerToken<'_> = owner
            .error_handler(|error| {
                let _ = error;
            })
            .expect("handler should register");
        let handler: ErrorHandler<'_> = token.view();
        let reporter: ErrorReporter<'_> = handler;
        let _ = reporter.handle(SilexError::recoverable(SilexErrorKind::Framework("compile-pass".to_string())));
    }).expect("child owner should initialize");
}
