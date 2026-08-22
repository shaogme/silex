#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{
    CallbackInvokeError, ComputationInitError, EffectPhase, ErrorHandlerToken, OwnerAccess, Runtime,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, &'static str> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn initial_error_disposes_the_provisional_memo_and_cleanup() {
    let mut runtime = Runtime::new();
    let callback_runs = Rc::new(Cell::new(0));
    let cleanup_runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal creation");
            let callback_runs_in_memo = callback_runs.clone();
            let cleanup_runs_in_memo = cleanup_runs.clone();
            let result = scope.computed(
                move || {
                    callback_runs_in_memo.set(callback_runs_in_memo.get() + 1);
                    let cleanup_runs = cleanup_runs_in_memo.clone();
                    scope
                        .on_cleanup(
                            move || {
                                cleanup_runs.set(cleanup_runs.get() + 1);
                                Ok(())
                            },
                            handler(scope),
                        )
                        .expect("memo cleanup registration");
                    let _ = source.get().expect("source read");
                    Err::<i32, _>("initial")
                },
                handler(scope),
            );

            assert!(matches!(
                result,
                Err(ComputationInitError::Initial("initial"))
            ));
            assert_eq!(callback_runs.get(), 1);
            assert_eq!(cleanup_runs.get(), 1);

            set_source.set(1).expect("source update");
            assert_eq!(callback_runs.get(), 1);
        })
        .expect("scope execution");
}

#[test]
fn explicit_reads_return_user_errors_and_retries_keep_the_last_success() {
    let mut runtime = Runtime::new();
    let should_fail = Rc::new(Cell::new(false));
    let previous_values = Rc::new(RefCell::new(Vec::new()));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal creation");
            let should_fail_in_memo = should_fail.clone();
            let previous_values_in_memo = previous_values.clone();
            let memo = scope
                .computed(
                    move || {
                        let source = source.get().expect("source read");
                        previous_values_in_memo.borrow_mut().push(source);
                        if should_fail_in_memo.get() {
                            Err("rejected")
                        } else {
                            Ok(source)
                        }
                    },
                    handler(scope),
                )
                .expect("memo creation");

            assert_eq!(memo.get(), Ok(0));
            should_fail.set(true);
            set_source.set(1).expect("source update");
            assert!(matches!(
                memo.get(),
                Err(CallbackInvokeError::User("rejected"))
            ));
            assert!(matches!(
                memo.get(),
                Err(CallbackInvokeError::User("rejected"))
            ));
            assert_eq!(previous_values.borrow().as_slice(), &[0, 1, 1]);

            should_fail.set(false);
            set_source.set(0).expect("equal recovery");
            assert_eq!(memo.get(), Ok(0));
            assert_eq!(previous_values.borrow().as_slice(), &[0, 1, 1, 0]);
        })
        .expect("scope execution");
}

#[test]
fn deferred_errors_use_the_handler_without_notifying_dependents() {
    let mut runtime = Runtime::new();
    let errors = Rc::new(RefCell::new(Vec::new()));
    let should_fail = Rc::new(Cell::new(false));
    let effect_runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal creation");
            let should_fail_in_memo = should_fail.clone();
            let memo_errors = errors.clone();
            let memo_handler = scope
                .error_handler(move |error| memo_errors.borrow_mut().push(error))
                .expect("memo handler registration");
            let memo = scope
                .computed(
                    move || {
                        let value = source.get().expect("source read");
                        if should_fail_in_memo.get() {
                            Err("rejected")
                        } else {
                            Ok(value)
                        }
                    },
                    memo_handler,
                )
                .expect("memo creation");
            let effect_runs_in_effect = effect_runs.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        memo.get().map(|_| ()).map_err(|error| match error {
                            CallbackInvokeError::User(error) => error,
                            CallbackInvokeError::Runtime(_) => "runtime",
                            CallbackInvokeError::Handler(_) => "runtime",
                        })?;
                        effect_runs_in_effect.set(effect_runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect creation");

            assert_eq!(effect_runs.get(), 1);
            should_fail.set(true);
            set_source.set(1).expect("failing source update");
            assert_eq!(errors.borrow().as_slice(), &["rejected"]);
            assert_eq!(effect_runs.get(), 1);

            should_fail.set(false);
            set_source.set(0).expect("equal recovery");
            assert_eq!(errors.borrow().as_slice(), &["rejected"]);
            assert_eq!(effect_runs.get(), 1);

            set_source.set(2).expect("changed recovery");
            assert_eq!(errors.borrow().as_slice(), &["rejected"]);
            assert_eq!(effect_runs.get(), 2);
        })
        .expect("scope execution");
}

#[test]
fn untracked_reads_do_not_subscribe_the_outer_effect_and_still_propagate_errors() {
    let mut runtime = Runtime::new();
    let effect_runs = Rc::new(Cell::new(0));
    let should_fail = Rc::new(Cell::new(false));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal creation");
            let should_fail_in_memo = should_fail.clone();
            let memo = scope
                .computed(
                    move || {
                        let value = source.get().expect("source read");
                        if should_fail_in_memo.get() {
                            Err("rejected")
                        } else {
                            Ok(value)
                        }
                    },
                    handler(scope),
                )
                .expect("memo creation");
            let effect_runs_in_effect = effect_runs.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        memo.with_untracked(|_| ()).map_err(|error| match error {
                            CallbackInvokeError::Runtime(_) => "runtime",
                            CallbackInvokeError::User(error) => error,
                            CallbackInvokeError::Handler(_) => "runtime",
                        })?;
                        effect_runs_in_effect.set(effect_runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect creation");

            assert_eq!(effect_runs.get(), 1);
            set_source.set(1).expect("source update");
            assert_eq!(effect_runs.get(), 1);

            should_fail.set(true);
            set_source.set(2).expect("failing source update");
            assert_eq!(effect_runs.get(), 1);
            assert!(matches!(
                memo.get_untracked(),
                Err(CallbackInvokeError::User("rejected"))
            ));
        })
        .expect("scope execution");
}

#[test]
fn failed_dynamic_dependencies_are_rolled_back_before_retry() {
    let mut runtime = Runtime::new();
    let should_fail = Rc::new(Cell::new(false));
    let callback_runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (use_right, set_use_right) = scope.signal(false).expect("selector creation");
            let (left, set_left) = scope.signal(1_i32).expect("left source creation");
            let (right, set_right) = scope.signal(10_i32).expect("right source creation");
            let should_fail_in_memo = should_fail.clone();
            let callback_runs_in_memo = callback_runs.clone();
            let memo = scope
                .computed(
                    move || {
                        callback_runs_in_memo.set(callback_runs_in_memo.get() + 1);
                        let value = if use_right.get().expect("selector read") {
                            right.get().expect("right read")
                        } else {
                            left.get().expect("left read")
                        };
                        if should_fail_in_memo.get() {
                            Err("rejected")
                        } else {
                            Ok(value)
                        }
                    },
                    handler(scope),
                )
                .expect("memo creation");

            assert_eq!(memo.get(), Ok(1));
            should_fail.set(true);
            set_use_right.set(true).expect("switch to failing branch");
            assert!(matches!(
                memo.get(),
                Err(CallbackInvokeError::User("rejected"))
            ));
            assert_eq!(callback_runs.get(), 2);

            should_fail.set(false);
            set_use_right
                .set(false)
                .expect("switch back to left branch");
            assert_eq!(memo.get(), Ok(1));
            assert_eq!(callback_runs.get(), 3);

            set_right.set(11).expect("right update");
            assert_eq!(memo.get(), Ok(1));
            assert_eq!(callback_runs.get(), 3);

            set_left.set(2).expect("left update");
            assert_eq!(memo.get(), Ok(2));
            assert_eq!(callback_runs.get(), 4);
        })
        .expect("scope execution");
}
