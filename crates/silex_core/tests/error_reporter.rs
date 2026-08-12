use silex_core::{ErrorHandler, ErrorReporter, Runtime, SilexError};
use std::{cell::RefCell, rc::Rc};

#[test]
fn reporter_delivers_errors_without_shared_context() {
    let first = Rc::new(RefCell::new(Vec::new()));
    let second = Rc::new(RefCell::new(Vec::new()));
    let first_for_reporter = first.clone();
    let second_for_reporter = second.clone();
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let first_reporter =
            scope.error_handler(move |error| first_for_reporter.borrow_mut().push(error));
        let second_reporter =
            scope.error_handler(move |error| second_for_reporter.borrow_mut().push(error));

        first_reporter
            .handle(SilexError::Framework("first".to_string()))
            .expect("first reporter should handle the error");
        second_reporter
            .handle(SilexError::Framework("second".to_string()))
            .expect("second reporter should handle the error");
    });

    assert!(matches!(
        first.borrow().as_slice(),
        [SilexError::Framework(message)] if message == "first"
    ));
    assert!(matches!(
        second.borrow().as_slice(),
        [SilexError::Framework(message)] if message == "second"
    ));
}

#[test]
fn reporter_can_capture_a_scoped_value() {
    let observed = Rc::new(RefCell::new(None));
    let observed_for_reporter = observed.clone();
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let reporter = scope.error_handler(move |error| {
            *observed_for_reporter.borrow_mut() = Some(error.to_string());
        });
        reporter
            .handle(SilexError::Javascript("scoped".to_string()))
            .expect("reporter should handle the error");
        assert!(scope.is_active());
    });

    assert_eq!(
        observed.borrow().as_deref(),
        Some("JavaScript Error: scoped")
    );
}

#[test]
fn error_reporter_is_the_reactivity_handler_alias() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let handler: ErrorHandler<'_, SilexError> = scope.error_handler(|_| {});
        let reporter: ErrorReporter<'_> = handler;

        reporter
            .handle(SilexError::Framework("alias".to_string()))
            .expect("reporter should handle the error");
    });
}
