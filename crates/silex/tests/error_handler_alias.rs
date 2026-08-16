use silex::{ErrorHandler, ErrorHandlerToken, ErrorReporter, Runtime, SilexError, SilexErrorKind};

#[test]
fn facade_exports_one_handler_type_under_both_names() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let token: ErrorHandlerToken<'_> = owner
                .error_handler(|_| {})
                .expect("error handler should be registered");
            let handler: ErrorHandler<'_> = token.view();
            let reporter: ErrorReporter<'_> = handler;

            reporter
                .handle(SilexError::recoverable(SilexErrorKind::Framework(
                    String::from("alias"),
                )))
                .expect("reporter should handle the error");
        })
        .expect("transient owner should close");
}
