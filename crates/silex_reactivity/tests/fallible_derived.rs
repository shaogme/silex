use silex_reactivity::{
    CallbackInvokeError, ComputationInitError, ErrorHandlerToken, OwnerAccess, Runtime,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, PartialEq)]
enum TestError {
    Rejected,
}

fn handler<'scope>(
    scope: OwnerAccess<'scope>,
    errors: Rc<RefCell<Vec<TestError>>>,
) -> ErrorHandlerToken<'scope, TestError> {
    scope
        .error_handler(move |error| errors.borrow_mut().push(error))
        .expect("handler registration")
}

#[test]
fn initial_error_is_returned_and_provisional_node_is_disposed() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let errors = Rc::new(RefCell::new(Vec::new()));
            let result = scope.computed_always(
                || Err::<i32, _>(TestError::Rejected),
                handler(scope, errors),
            );
            assert!(matches!(
                result,
                Err(ComputationInitError::Initial(TestError::Rejected))
            ));
            assert_eq!(scope.signal(1_i32).expect("signal creation").0.get(), Ok(1));
        })
        .expect("test operation should succeed");
}

#[test]
fn read_returns_user_error_without_using_the_previous_value() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let errors = Rc::new(RefCell::new(Vec::new()));
            let (source, set_source) = scope.signal(0_i32).expect("signal creation");
            let should_fail = Rc::new(RefCell::new(false));
            let should_fail_in_callback = should_fail.clone();
            let derived = scope
                .computed_always(
                    move || {
                        let _ = source.get().expect("source read");
                        if *should_fail_in_callback.borrow() {
                            Err(TestError::Rejected)
                        } else {
                            Ok(7_i32)
                        }
                    },
                    handler(scope, errors.clone()),
                )
                .expect("derived creation");

            assert_eq!(derived.get(), Ok(7));
            *should_fail.borrow_mut() = true;
            set_source.set(1).expect("source update");
            let result = derived.get();
            assert!(matches!(
                result,
                Err(CallbackInvokeError::User(TestError::Rejected))
            ));
            assert!(matches!(
                derived.get(),
                Err(CallbackInvokeError::User(TestError::Rejected))
            ));
            assert!(errors.borrow().is_empty());
        })
        .expect("test operation should succeed");
}

#[test]
fn deferred_error_is_dispatched_and_next_read_can_retry() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let errors = Rc::new(RefCell::new(Vec::new()));
            let (source, set_source) = scope.signal(1_i32).expect("signal creation");
            let derived = scope
                .computed_always(
                    move || {
                        if source.get().map_err(|_| TestError::Rejected)? == 2 {
                            Err(TestError::Rejected)
                        } else {
                            Ok(1_i32)
                        }
                    },
                    handler(scope, errors.clone()),
                )
                .expect("derived creation");
            let effect_derived = derived;
            scope
                .effect(
                    move || {
                        effect_derived
                            .get()
                            .map(|_| ())
                            .map_err(|error| match error {
                                CallbackInvokeError::User(error) => error,
                                CallbackInvokeError::Runtime(_) => TestError::Rejected,
                                CallbackInvokeError::Handler(_) => TestError::Rejected,
                                CallbackInvokeError::Close(_) => TestError::Rejected,
                            })
                    },
                    handler(scope, errors.clone()),
                )
                .expect("effect creation");
            assert_eq!(derived.get(), Ok(1));

            set_source.set(2).expect("source update");
            assert_eq!(errors.borrow().as_slice(), &[TestError::Rejected]);
            assert!(matches!(
                derived.get(),
                Err(CallbackInvokeError::User(TestError::Rejected))
            ));

            set_source.set(1).expect("source recovery");
            assert_eq!(derived.get(), Ok(1));
        })
        .expect("test operation should succeed");
}
