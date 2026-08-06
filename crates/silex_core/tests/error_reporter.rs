use silex_core::{ErrorReporter, Runtime, SilexError};
use std::{cell::RefCell, rc::Rc};

#[test]
fn reporter_delivers_errors_without_shared_context() {
    let first = Rc::new(RefCell::new(Vec::new()));
    let second = Rc::new(RefCell::new(Vec::new()));
    let first_for_reporter = first.clone();
    let second_for_reporter = second.clone();
    let first_reporter =
        ErrorReporter::new(move |error| first_for_reporter.borrow_mut().push(error));
    let second_reporter =
        ErrorReporter::new(move |error| second_for_reporter.borrow_mut().push(error));

    first_reporter.report(SilexError::Framework("first".to_string()));
    second_reporter.report(SilexError::Framework("second".to_string()));

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
        let reporter = ErrorReporter::new(move |error| {
            *observed_for_reporter.borrow_mut() = Some(error.to_string());
        });
        reporter.report(SilexError::Javascript("scoped".to_string()));
        assert!(scope.is_active());
    });

    assert_eq!(
        observed.borrow().as_deref(),
        Some("JavaScript Error: scoped")
    );
}
