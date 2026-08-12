use silex::{ErrorHandler, ErrorReporter, Runtime, SilexError};

#[test]
fn facade_exports_one_handler_type_under_both_names() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let handler: ErrorHandler<'_, SilexError> = scope
            .error_handler(|_| {})
            .expect("error handler should be registered");
        let reporter: ErrorReporter<'_> = handler;

        reporter
            .handle(SilexError::Framework(String::from("alias")))
            .expect("reporter should handle the error");
    });
}
