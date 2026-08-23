#![allow(clippy::expect_used)]

use silex_reactivity::{EffectPhase, ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime};
use std::{cell::Cell, rc::Rc};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ReactiveError> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn read_guard_dereferences_and_blocks_writes_until_finished() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let signal = scope.signal((1_i32, 2_i32)).expect("signal creation");
            let first = signal.read_signal().read().expect("read guard");
            let second = signal.read_signal().read().expect("second read guard");

            assert_eq!(first.0, 1);
            assert_eq!(second.1, 2);
            assert!(matches!(
                signal.write_signal().write(),
                Err(ReactiveError::BorrowConflict)
            ));

            first.finish().expect("first guard finish");
            second.finish().expect("second guard finish");
            let mut write = signal.write_signal().write().expect("write guard");
            write.0 = 3;
            write.commit().expect("write commit");
            assert_eq!(signal.get().expect("signal read"), (3, 2));
        })
        .expect("runtime scope");
}

#[test]
fn write_guard_abort_and_drop_have_explicit_fallbacks() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let signal = scope.signal(1_i32).expect("signal creation");
            let mut aborted = signal.write_signal().write().expect("write guard");
            *aborted = 2;
            aborted.abort().expect("write abort");
            assert_eq!(signal.get().expect("signal read"), 2);

            {
                let mut dropped = signal.write_signal().write().expect("write guard");
                *dropped = 3;
            }
            assert_eq!(signal.get().expect("signal read"), 3);
        })
        .expect("runtime scope");
}

#[test]
fn tracked_and_untracked_read_guards_preserve_dependency_semantics() {
    let mut runtime = Runtime::new();
    let tracked_runs = Rc::new(Cell::new(0));
    let untracked_runs = Rc::new(Cell::new(0));
    runtime
        .with_transient(|scope| {
            let source = scope.signal(1_i32).expect("signal creation");
            let tracked_runs_in_effect = tracked_runs.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        let guard = source.read_signal().read()?;
                        std::hint::black_box(&*guard);
                        guard.finish()?;
                        tracked_runs_in_effect.set(tracked_runs_in_effect.get() + 1);
                        Ok::<(), ReactiveError>(())
                    },
                    handler(scope),
                )
                .expect("tracked effect");

            let untracked_runs_in_effect = untracked_runs.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        let guard = source.read_signal().read_untracked()?;
                        std::hint::black_box(&*guard);
                        guard.finish()?;
                        untracked_runs_in_effect.set(untracked_runs_in_effect.get() + 1);
                        Ok::<(), ReactiveError>(())
                    },
                    handler(scope),
                )
                .expect("untracked effect");

            assert_eq!(tracked_runs.get(), 1);
            assert_eq!(untracked_runs.get(), 1);
            source.set(2).expect("source update");
            assert_eq!(tracked_runs.get(), 2);
            assert_eq!(untracked_runs.get(), 1);
        })
        .expect("runtime scope");
}
