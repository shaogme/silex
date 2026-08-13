use silex_core::{
    ErrorHandler, ReactiveError, Runtime, Scope, SilexError, SilexErrorKind, SilexResult,
    WatchOptions, runtime_inputs_of,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn collecting_handler<'scope>(
    scope: Scope<'scope>,
    errors: Rc<RefCell<Vec<SilexError>>>,
) -> ErrorHandler<'scope, SilexError> {
    scope
        .error_handler(move |error| errors.borrow_mut().push(error))
        .expect("error handler should register")
}

#[test]
fn registration_error_maps_to_reactivity_error() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let inputs = first
        .child(|scope| {
            let (source, _) = scope.signal(1_i32).expect("signal should initialize");
            runtime_inputs_of(source)
        })
        .expect("child scope should initialize");

    let result = second
        .child(|scope| {
            scope
                .effect_from(
                    inputs,
                    || Ok(()),
                    collecting_handler(scope, Rc::new(RefCell::new(Vec::new()))),
                )
                .map(|_| ())
        })
        .expect("child scope should initialize");

    assert!(matches!(
        result,
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::RuntimeMismatch,
        )))
    ));
}

#[test]
fn initial_silex_error_is_returned_without_reporting_twice() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));

    runtime
        .child(|scope| {
            let result = scope.effect(
                || {
                    Err(SilexError::fatal(SilexErrorKind::Dom(
                        "initial".to_string(),
                    )))
                },
                collecting_handler(scope, errors.clone()),
            );

            assert!(matches!(
                result,
                Err(SilexError::Fatal(SilexErrorKind::Dom(message))) if message == "initial"
            ));
            assert!(errors.borrow().is_empty());
        })
        .expect("child scope should initialize");
}

#[test]
fn deferred_error_reaches_reporter_and_effect_can_retry() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let should_fail = Rc::new(Cell::new(false));
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal should initialize");
            let should_fail_in_effect = should_fail.clone();
            let runs_in_effect = runs.clone();
            let effect = scope
                .effect(
                    move || -> SilexResult<()> {
                        source.get()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        if should_fail_in_effect.get() {
                            Err(SilexError::recoverable(SilexErrorKind::Framework("deferred".to_string())))
                        } else {
                            Ok(())
                        }
                    },
                    collecting_handler(scope, errors.clone()),
                )
                .expect("effect should initialize");

            should_fail.set(true);
            set_source.set(1).expect("signal should be writable");
            assert!(matches!(
                errors.borrow().as_slice(),
                [SilexError::Recoverable(SilexErrorKind::Framework(message))] if message == "deferred"
            ));

            should_fail.set(false);
            set_source.set(2).expect("signal should be writable");
            assert_eq!(runs.get(), 3);
            assert!(matches!(effect.stop(), Ok(true)));
        })
        .expect("child scope should initialize");
}

#[test]
fn previous_error_preserves_the_last_successful_value() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let previous_values = Rc::new(RefCell::new(Vec::new()));
    let fail_next = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal should initialize");
            let previous_values_in_effect = previous_values.clone();
            let fail_next_in_effect = fail_next.clone();
            scope
                .effect_with_previous(
                    move |previous| {
                        source.get()?;
                        previous_values_in_effect
                            .borrow_mut()
                            .push(previous.copied());
                        if fail_next_in_effect.replace(false) {
                            Err(SilexError::recoverable(SilexErrorKind::Framework("previous".to_string())))
                        } else {
                            Ok(previous.copied().unwrap_or(0) + 1)
                        }
                    },
                    collecting_handler(scope, errors.clone()),
                )
                .expect("previous effect should initialize");

            fail_next.set(true);
            set_source.set(1).expect("signal should be writable");
            set_source.set(2).expect("signal should be writable");
            assert_eq!(
                previous_values.borrow().as_slice(),
                &[None, Some(1), Some(1)]
            );
            assert!(matches!(
                errors.borrow().as_slice(),
                [SilexError::Recoverable(SilexErrorKind::Framework(message))] if message == "previous"
            ));
        })
        .expect("child scope should initialize");
}

#[test]
fn watch_error_preserves_the_previous_snapshot_for_retry() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let fail_next = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal should initialize");
            let calls_in_callback = calls.clone();
            let fail_next_in_callback = fail_next.clone();
            scope
                .watch_getter_with_options(
                    move || -> SilexResult<i32> { source.get() },
                    move |new, old| {
                        if fail_next_in_callback.replace(false) {
                            Err(SilexError::recoverable(SilexErrorKind::Framework(
                                "watch".to_string(),
                            )))
                        } else {
                            calls_in_callback.borrow_mut().push((*new, old.copied()));
                            Ok(())
                        }
                    },
                    collecting_handler(scope, errors.clone()),
                    WatchOptions::default(),
                )
                .expect("watch should initialize");

            fail_next.set(true);
            set_source.set(1).expect("signal should be writable");
            assert!(errors.borrow().iter().any(|error| matches!(
                error,
                SilexError::Recoverable(SilexErrorKind::Framework(message)) if message == "watch"
            )));
            assert!(calls.borrow().is_empty());

            set_source.set(2).expect("signal should be writable");
            assert_eq!(calls.borrow().as_slice(), &[(2, Some(0))]);
        })
        .expect("child scope should initialize");
}

#[test]
fn reporter_handler_can_be_cloned_for_effect_and_cleanup() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    runtime
        .child(|scope| {
            let handler = scope
                .error_handler({
                    let errors = errors.clone();
                    move |error| errors.borrow_mut().push(error)
                })
                .expect("error handler should register");
            scope
                .effect(|| Ok(()), handler)
                .expect("effect should initialize");
            scope
                .on_cleanup(
                    || {
                        Err(SilexError::recoverable(SilexErrorKind::Framework(
                            "cleanup".to_string(),
                        )))
                    },
                    handler,
                )
                .expect("cleanup should register");
        })
        .expect("child scope should initialize");

    assert!(matches!(
        errors.borrow().as_slice(),
        [SilexError::Recoverable(SilexErrorKind::Framework(message))] if message == "cleanup"
    ));
}
