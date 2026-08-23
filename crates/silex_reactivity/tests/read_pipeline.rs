use silex_reactivity::{
    CallbackInvokeError, EffectPhase, ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime,
};
use std::cell::Cell;
use std::rc::Rc;

fn runtime_handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ReactiveError> {
    scope.error_handler(|_| {}).expect("handler registration")
}

fn callback_handler<'scope>(
    scope: OwnerAccess<'scope>,
) -> ErrorHandlerToken<'scope, CallbackInvokeError<ReactiveError>> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[derive(Clone, Debug, PartialEq)]
enum TestError {
    Rejected,
}

#[test]
fn signal_track_supports_non_clone_values_and_reacts() {
    struct NonClone(i32);

    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    runtime
        .with_transient(|scope| {
            let source = scope.signal(NonClone(1)).expect("source creation");
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.track()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    runtime_handler(scope),
                )
                .expect("effect creation");

            assert_eq!(runs.get(), 1);
            source.update(|value| value.0 += 1).expect("source update");
            assert_eq!(runs.get(), 2);
        })
        .expect("runtime scope");
}

#[test]
fn signal_track_uses_its_read_capability() {
    struct NonClone(i32);

    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let signal = scope.signal(NonClone(1)).expect("signal creation");
            signal.track().expect("rw signal track");
            signal.update(|value| value.0 += 1).expect("signal update");
            signal.track().expect("rw signal track after update");
        })
        .expect("runtime scope");
}

#[test]
fn stored_track_validates_without_reading_a_non_clone_payload() {
    struct NonClone(i32);

    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let stored = scope.stored(NonClone(1)).expect("stored creation");
            stored.track().expect("stored track");
            stored
                .with(|value| {
                    assert_eq!(value.0, 1);
                    stored.track().expect("nested stored track");
                })
                .expect("stored read");
            stored
                .with_untracked(|value| assert_eq!(value.0, 1))
                .expect("stored untracked read");
        })
        .expect("runtime scope");
}

#[test]
fn untracked_signal_read_does_not_subscribe_an_effect() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    let seen = Rc::new(Cell::new(0));
    runtime
        .with_transient(|scope| {
            let source = scope.signal(1_i32).expect("source creation");
            let runs_in_effect = runs.clone();
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source
                            .read_signal()
                            .with_untracked(|value| seen_in_effect.set(*value))?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    runtime_handler(scope),
                )
                .expect("effect creation");

            assert_eq!(runs.get(), 1);
            assert_eq!(seen.get(), 1);
            source.set(2).expect("source update");
            assert_eq!(runs.get(), 1);
            assert_eq!(seen.get(), 1);
        })
        .expect("runtime scope");
}

#[test]
fn computed_track_evaluates_and_subscribes_once_per_read() {
    let mut runtime = Runtime::new();
    let evaluations = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));
    runtime
        .with_transient(|scope| {
            let source = scope.signal(1_i32).expect("source creation");
            let evaluations_in_computed = evaluations.clone();
            let computed = scope
                .computed(
                    move || {
                        evaluations_in_computed.set(evaluations_in_computed.get() + 1);
                        source.get().map(|value| value + 1)
                    },
                    runtime_handler(scope),
                )
                .expect("computed creation");
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        computed.track()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    callback_handler(scope),
                )
                .expect("effect creation");

            assert_eq!(evaluations.get(), 1);
            assert_eq!(runs.get(), 1);
            source.set(2).expect("source update");
            assert_eq!(evaluations.get(), 2);
            assert_eq!(runs.get(), 2);
        })
        .expect("runtime scope");
}

#[test]
fn fallible_computed_track_preserves_user_errors() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let should_fail = scope.signal(false).expect("source creation");
            let computed = scope
                .computed_always(
                    move || {
                        if should_fail.get().map_err(|_| TestError::Rejected)? {
                            Err(TestError::Rejected)
                        } else {
                            Ok(1_i32)
                        }
                    },
                    scope.error_handler(|_| {}).expect("handler registration"),
                )
                .expect("computed creation");

            assert_eq!(computed.track(), Ok(()));
            should_fail.set(true).expect("source update");
            assert_eq!(
                computed.track(),
                Err(CallbackInvokeError::User(TestError::Rejected))
            );
        })
        .expect("runtime scope");
}
