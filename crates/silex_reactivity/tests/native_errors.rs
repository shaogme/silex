#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{ComputationInitError, ErrorHandlerToken, OwnerAccess, Runtime};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn collecting_handler<'scope>(
    scope: OwnerAccess<'scope>,
    errors: Rc<RefCell<Vec<&'static str>>>,
) -> ErrorHandlerToken<'scope, &'static str> {
    scope
        .error_handler(move |error| errors.borrow_mut().push(error))
        .expect("handler registration")
}

#[test]
fn initial_callback_error_returns_without_calling_the_handler() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let cleanup_runs = Rc::new(Cell::new(0));
    let callback_runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("fallible reactive creation");
            let cleanup_runs_in_callback = cleanup_runs.clone();
            let callback_runs_in_callback = callback_runs.clone();
            let cleanup_token = collecting_handler(scope, errors.clone());
            let cleanup_handler = cleanup_token.view();
            let result = scope.effect(
                move || {
                    callback_runs_in_callback.set(callback_runs_in_callback.get() + 1);
                    source.get().expect("test operation should succeed");
                    let cleanup_runs = cleanup_runs_in_callback.clone();
                    scope
                        .on_cleanup(
                            move || {
                                cleanup_runs.set(cleanup_runs.get() + 1);
                                Ok(())
                            },
                            cleanup_handler,
                        )
                        .expect("provisional cleanup should register");
                    Err("initial")
                },
                collecting_handler(scope, errors.clone()),
            );

            assert!(matches!(
                result,
                Err(ComputationInitError::Initial("initial"))
            ));
            assert!(errors.borrow().is_empty());
            assert_eq!(cleanup_runs.get(), 1);

            set_source.set(1).expect("test operation should succeed");
            assert_eq!(callback_runs.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn initial_failure_does_not_reenter_from_rollback_cleanup() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let callback_runs = Rc::new(Cell::new(0));
    let register_cleanup = Rc::new(Cell::new(true));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("fallible reactive creation");
            let callback_runs_in_callback = callback_runs.clone();
            let register_cleanup_in_callback = register_cleanup.clone();
            let setter_in_cleanup = set_source;
            let cleanup_token = collecting_handler(scope, errors.clone());
            let cleanup_handler = cleanup_token.view();
            let result = scope.effect(
                move || {
                    callback_runs_in_callback.set(callback_runs_in_callback.get() + 1);
                    source.get().expect("test operation should succeed");
                    if register_cleanup_in_callback.replace(false) {
                        scope
                            .on_cleanup(
                                move || {
                                    setter_in_cleanup
                                        .set(1)
                                        .expect("test operation should succeed");
                                    Ok(())
                                },
                                cleanup_handler,
                            )
                            .expect("rollback cleanup should register");
                    }
                    Err("initial")
                },
                collecting_handler(scope, errors.clone()),
            );

            assert!(matches!(
                result,
                Err(ComputationInitError::Initial("initial"))
            ));
            assert_eq!(callback_runs.get(), 1);
            assert!(errors.borrow().is_empty());
        })
        .expect("test operation should succeed");
}

#[test]
fn nested_node_cleanup_errors_wait_for_outer_run_recovery() {
    let mut runtime = Runtime::new();
    let registered_cleanup_runs = Rc::new(Cell::new(0));
    let first_run = Rc::new(Cell::new(true));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("fallible reactive creation");
            let registered_cleanup_runs_in_handler = registered_cleanup_runs.clone();
            let scope_in_handler = scope;
            let recovered_cleanup_token = scope
                .error_handler(|_: &'static str| {})
                .expect("handler registration");
            let recovered_cleanup_handler = recovered_cleanup_token.view();
            let nested_effect_token = scope
                .error_handler(|_: ()| {})
                .expect("handler registration");
            let nested_effect_handler = nested_effect_token.view();
            let outer_effect_token = scope
                .error_handler(|_: ()| {})
                .expect("handler registration");
            let outer_effect_handler = outer_effect_token.view();
            let cleanup_error_token = scope
                .error_handler(move |_: &'static str| {
                    let registered_cleanup_runs = registered_cleanup_runs_in_handler.clone();
                    scope_in_handler
                        .on_cleanup(
                            move || {
                                registered_cleanup_runs.set(registered_cleanup_runs.get() + 1);
                                Ok(())
                            },
                            recovered_cleanup_handler,
                        )
                        .expect("recovered owner should accept a root cleanup");
                })
                .expect("handler registration");
            let cleanup_error_handler = cleanup_error_token.view();
            let first_run_in_effect = first_run.clone();
            let cleanup_error_handler_in_effect = cleanup_error_handler;
            scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
                        if first_run_in_effect.replace(false) {
                            let nested_cleanup_handler = cleanup_error_handler_in_effect;
                            scope
                                .effect(
                                    move || {
                                        scope
                                            .on_cleanup(
                                                || Err("nested cleanup"),
                                                nested_cleanup_handler,
                                            )
                                            .expect("nested cleanup should register");
                                        Ok(())
                                    },
                                    nested_effect_handler,
                                )
                                .expect("nested effect should initialize");
                        }
                        Ok(())
                    },
                    outer_effect_handler,
                )
                .expect("outer effect should initialize");

            set_source.set(1).expect("test operation should succeed");
            set_source.set(2).expect("test operation should succeed");
            assert_eq!(registered_cleanup_runs.get(), 0);
        })
        .expect("test operation should succeed");

    assert_eq!(registered_cleanup_runs.get(), 1);
}

#[test]
fn deferred_callback_error_reaches_its_handler_and_can_retry() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let callback_runs = Rc::new(Cell::new(0));
    let should_fail = Rc::new(Cell::new(false));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("fallible reactive creation");
            let callback_runs_in_callback = callback_runs.clone();
            let should_fail_in_callback = should_fail.clone();
            let effect = scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
                        callback_runs_in_callback.set(callback_runs_in_callback.get() + 1);
                        if should_fail_in_callback.get() {
                            Err("deferred")
                        } else {
                            Ok(())
                        }
                    },
                    collecting_handler(scope, errors.clone()),
                )
                .expect("effect should initialize");

            should_fail.set(true);
            set_source.set(1).expect("test operation should succeed");
            assert_eq!(errors.borrow().as_slice(), &["deferred"]);
            assert_eq!(callback_runs.get(), 2);

            should_fail.set(false);
            set_source.set(2).expect("test operation should succeed");
            assert_eq!(errors.borrow().as_slice(), &["deferred"]);
            assert_eq!(callback_runs.get(), 3);
            assert_eq!(effect.stop(), Ok(true));
        })
        .expect("test operation should succeed");
}

#[test]
fn failed_dynamic_run_rolls_back_new_dependency_edges() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let runs = Rc::new(Cell::new(0));
    let fail_next = Rc::new(Cell::new(false));

    runtime
        .with_transient(|scope| {
            let (switch, set_switch) = scope.signal(false).expect("fallible reactive creation");
            let (left, set_left) = scope.signal(0_i32).expect("fallible reactive creation");
            let (right, set_right) = scope.signal(0_i32).expect("fallible reactive creation");
            let runs_in_callback = runs.clone();
            let fail_next_in_callback = fail_next.clone();
            scope
                .effect(
                    move || {
                        let value = if switch.get().expect("reactive read") {
                            right.get().expect("reactive read")
                        } else {
                            left.get().expect("reactive read")
                        };
                        std::hint::black_box(value);
                        runs_in_callback.set(runs_in_callback.get() + 1);
                        if fail_next_in_callback.replace(false) {
                            Err("dynamic")
                        } else {
                            Ok(())
                        }
                    },
                    collecting_handler(scope, errors.clone()),
                )
                .expect("effect should initialize");

            fail_next.set(true);
            set_switch.set(true).expect("test operation should succeed");
            assert_eq!(errors.borrow().as_slice(), &["dynamic"]);
            assert_eq!(runs.get(), 2);

            set_left.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 3);
            set_right.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 4);
        })
        .expect("test operation should succeed");
}

#[test]
fn previous_value_is_kept_when_a_run_returns_an_error() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let previous_values = Rc::new(RefCell::new(Vec::new()));
    let fail_next = Rc::new(Cell::new(false));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("fallible reactive creation");
            let previous_values_in_callback = previous_values.clone();
            let fail_next_in_callback = fail_next.clone();
            scope
                .effect_with_previous(
                    move |previous| {
                        source.get().expect("test operation should succeed");
                        previous_values_in_callback
                            .borrow_mut()
                            .push(previous.copied());
                        if fail_next_in_callback.replace(false) {
                            Err("previous")
                        } else {
                            Ok(previous.copied().unwrap_or(0) + 1)
                        }
                    },
                    collecting_handler(scope, errors.clone()),
                )
                .expect("previous effect should initialize");

            fail_next.set(true);
            set_source.set(1).expect("test operation should succeed");
            assert_eq!(errors.borrow().as_slice(), &["previous"]);
            set_source.set(2).expect("test operation should succeed");
            assert_eq!(
                previous_values.borrow().as_slice(),
                &[None, Some(1), Some(1)]
            );
        })
        .expect("test operation should succeed");
}

#[test]
fn watch_error_keeps_the_previous_snapshot_for_retry() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let calls = Rc::new(RefCell::new(Vec::new()));
    let fail_next = Rc::new(Cell::new(false));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("fallible reactive creation");
            let calls_in_callback = calls.clone();
            let fail_next_in_callback = fail_next.clone();
            scope
                .watch_getter(
                    move || Ok(source.get().expect("reactive read")),
                    move |new, old| {
                        if fail_next_in_callback.replace(false) {
                            Err("watch")
                        } else {
                            calls_in_callback.borrow_mut().push((*new, old.copied()));
                            Ok(())
                        }
                    },
                    collecting_handler(scope, errors.clone()),
                )
                .expect("watch should initialize");

            fail_next.set(true);
            set_source.set(1).expect("test operation should succeed");
            assert_eq!(errors.borrow().as_slice(), &["watch"]);
            assert!(calls.borrow().is_empty());

            set_source.set(2).expect("test operation should succeed");
            assert_eq!(calls.borrow().as_slice(), &[(2, Some(0))]);
        })
        .expect("test operation should succeed");
}

#[test]
fn cleanup_errors_do_not_skip_the_remaining_cleanup_batch() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let second_cleanup_ran = Rc::new(Cell::new(false));

    runtime
        .with_transient(|scope| {
            scope
                .on_cleanup(|| Err("cleanup"), collecting_handler(scope, errors.clone()))
                .expect("cleanup should register");
            let second_cleanup_ran_in_cleanup = second_cleanup_ran.clone();
            scope
                .on_cleanup(
                    move || {
                        second_cleanup_ran_in_cleanup.set(true);
                        Ok(())
                    },
                    collecting_handler(scope, errors.clone()),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    assert_eq!(errors.borrow().as_slice(), &["cleanup"]);
    assert!(second_cleanup_ran.get());
}

#[test]
fn final_cleanup_error_dispatch_can_access_a_stored_value() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let stored = scope.stored(1_i32).expect("fallible reactive creation");
            let observed_in_handler = observed.clone();
            let handler = scope
                .error_handler(move |_: &'static str| {
                    observed_in_handler.set(
                        stored
                            .update(|value| {
                                *value = 2;
                                *value
                            })
                            .expect("stored value should survive error dispatch"),
                    );
                })
                .expect("handler registration");
            scope
                .on_cleanup(|| Err("final cleanup"), handler)
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    assert_eq!(observed.get(), 2);
}
