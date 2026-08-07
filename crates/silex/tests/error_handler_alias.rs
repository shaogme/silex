use silex::{ErrorHandler, ErrorReporter, SilexError};

#[test]
fn facade_exports_one_handler_type_under_both_names() {
    let handler: ErrorHandler<'_, SilexError> = ErrorHandler::new(|_| {});
    let reporter: ErrorReporter<'_> = handler.clone();

    reporter.handle(SilexError::Framework(String::from("alias")));
}
