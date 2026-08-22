#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{
    ComputationInitError, EffectHandle, EffectPhase, ErrorHandlerToken, OwnerAccess, Runtime,
    WatchOptions,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn getter_watch_commits_values_and_gates_equal_updates() {
    let mut runtime = Runtime::new();
    let getter_runs = Rc::new(Cell::new(0));
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(1_i32).expect("fallible reactive creation");
            let getter_runs_in_getter = getter_runs.clone();
            let calls_in_callback = calls.clone();
            scope
                .watch_getter(
                    EffectPhase::Normal,
                    move || {
                        getter_runs_in_getter.set(getter_runs_in_getter.get() + 1);
                        Ok(source.get().expect("reactive read"))
                    },
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("watch should initialize");

            assert_eq!(getter_runs.get(), 1);
            assert!(calls.borrow().is_empty());
            source.set(1).expect("test operation should succeed");
            assert_eq!(getter_runs.get(), 2);
            assert!(calls.borrow().is_empty());
            source.set(2).expect("test operation should succeed");
            assert_eq!(calls.borrow().as_slice(), &[(2, Some(1))]);
        })
        .expect("test operation should succeed");
}

#[test]
fn immediate_once_watch_stops_after_the_initial_callback() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(1_i32).expect("fallible reactive creation");
            let calls_in_callback = calls.clone();
            let watcher = scope
                .watch_getter_with_options(
                    EffectPhase::Normal,
                    move || Ok(source.get().expect("reactive read")),
                    move |new, old| {
                        assert_eq!(*new, 1);
                        assert!(old.is_none());
                        calls_in_callback.set(calls_in_callback.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                    WatchOptions::default().immediate().once(),
                )
                .expect("watch should initialize");

            assert_eq!(calls.get(), 1);
            assert_eq!(watcher.stop(), Ok(false));
            source.set(2).expect("test operation should succeed");
            assert_eq!(calls.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn callback_reads_are_untracked_and_dynamic_getter_dependencies_replace() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let switch = scope.signal(true).expect("fallible reactive creation");
            let left = scope.signal(1_i32).expect("fallible reactive creation");
            let right = scope.signal(10_i32).expect("fallible reactive creation");
            let probe = scope.signal(0_i32).expect("fallible reactive creation");
            let calls_in_callback = calls.clone();
            scope
                .watch_getter(
                    EffectPhase::Normal,
                    move || {
                        if switch.get().expect("reactive read") {
                            Ok(left.get().expect("reactive read"))
                        } else {
                            Ok(right.get().expect("reactive read"))
                        }
                    },
                    move |_, _| {
                        probe.get().expect("test operation should succeed");
                        calls_in_callback.set(calls_in_callback.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("watch should initialize");

            probe.set(1).expect("test operation should succeed");
            assert_eq!(calls.get(), 0);
            right.set(11).expect("test operation should succeed");
            assert_eq!(calls.get(), 0);
            left.set(2).expect("test operation should succeed");
            assert_eq!(calls.get(), 1);
            probe.set(2).expect("test operation should succeed");
            assert_eq!(calls.get(), 1);
            switch.set(false).expect("test operation should succeed");
            assert_eq!(calls.get(), 2);
            left.set(3).expect("test operation should succeed");
            assert_eq!(calls.get(), 2);
            right.set(12).expect("test operation should succeed");
            assert_eq!(calls.get(), 3);
        })
        .expect("test operation should succeed");
}

#[test]
fn stop_cancels_future_runs_and_runs_cleanup_once() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(Cell::new(0));
    let cleanups = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0_i32).expect("fallible reactive creation");
            let calls_in_callback = calls.clone();
            let cleanups_in_callback = cleanups.clone();
            let watcher_scope = scope;
            let watcher = scope
                .watch_getter(
                    EffectPhase::Normal,
                    move || Ok(source.get().expect("reactive read")),
                    move |_, _| {
                        calls_in_callback.set(calls_in_callback.get() + 1);
                        let cleanups = cleanups_in_callback.clone();
                        watcher_scope
                            .on_cleanup(
                                move || {
                                    cleanups.set(cleanups.get() + 1);
                                    Ok(())
                                },
                                handler(watcher_scope),
                            )
                            .expect("watch cleanup should register");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("watch should initialize");

            source.set(1).expect("test operation should succeed");
            assert_eq!(calls.get(), 1);
            source.set(2).expect("test operation should succeed");
            assert_eq!(cleanups.get(), 1);
            assert_eq!(watcher.stop(), Ok(true));
            assert_eq!(cleanups.get(), 2);
            assert_eq!(watcher.stop(), Ok(false));
            source.set(3).expect("test operation should succeed");
            assert_eq!(calls.get(), 2);
        })
        .expect("test operation should succeed");

    assert_eq!(cleanups.get(), 2);
}

#[test]
fn callback_panic_keeps_the_old_snapshot_for_a_later_retry() {
    let mut runtime = Runtime::new();
    let should_panic = Rc::new(Cell::new(true));
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0_i32).expect("fallible reactive creation");
            let should_panic_in_callback = should_panic.clone();
            let calls_in_callback = calls.clone();
            scope
                .watch_getter(
                    EffectPhase::Normal,
                    move || Ok(source.get().expect("reactive read")),
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        if should_panic_in_callback.replace(false) {
                            panic!("watch callback panic");
                        }
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("watch should initialize");

            let panic = catch_unwind(AssertUnwindSafe(|| source.set(1)));
            assert!(panic.is_err());
            source.set(2).expect("test operation should succeed");
            assert_eq!(calls.borrow().as_slice(), &[(1, Some(0)), (2, Some(0))]);
        })
        .expect("test operation should succeed");
}

#[test]
fn initial_watch_panic_rolls_back_the_registered_node() {
    let mut runtime = Runtime::new();
    let getter_runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0_i32).expect("fallible reactive creation");
            let getter_runs_in_getter = getter_runs.clone();
            let panic = catch_unwind(AssertUnwindSafe(|| {
                scope
                    .watch_getter(
                        EffectPhase::Normal,
                        move || {
                            getter_runs_in_getter.set(getter_runs_in_getter.get() + 1);
                            let value = source.get().expect("reactive read");
                            if value == 0 {
                                panic!("initial watch panic");
                            }
                            Ok(value)
                        },
                        |_, _| Ok(()),
                        handler(scope),
                    )
                    .expect("test operation should succeed");
            }));
            assert!(panic.is_err());
            source.set(1).expect("test operation should succeed");
            assert_eq!(getter_runs.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn foreign_watch_reads_fail_before_callback_execution() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let first_root = first.owner().expect("first root");
    let second_root = second.owner().expect("second root");
    let getter_runs = Rc::new(Cell::new(0));
    let callback_runs = Rc::new(Cell::new(0));

    let result = first_root.with_access(|foreign_scope| {
        let foreign_source = foreign_scope
            .signal(1_i32)
            .expect("foreign signal should initialize");
        let _ = foreign_source;
        second_root.with_access(|scope| {
            let getter_runs_in_getter = getter_runs.clone();
            let callback_runs_in_callback = callback_runs.clone();
            scope
                .watch_getter(
                    EffectPhase::Normal,
                    move || {
                        getter_runs_in_getter.set(getter_runs_in_getter.get() + 1);
                        foreign_source.get().map_err(|_| ())
                    },
                    move |_, _| {
                        callback_runs_in_callback.set(callback_runs_in_callback.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .map(|_| ())
        })
    });

    assert!(matches!(result, Err(ComputationInitError::Initial(()))));
    assert_eq!(getter_runs.get(), 1);
    assert_eq!(callback_runs.get(), 0);

    second_root.close().expect("second root disposal");
    first_root.close().expect("first root disposal");
}

#[test]
fn owner_disposal_makes_a_watcher_handle_stopped() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    {
        let scope = root.access();
        let owner = scope.create_child().expect("fallible reactive creation");
        let watcher = owner
            .access()
            .watch_getter(
                EffectPhase::Normal,
                || Ok(1_i32),
                |_, _| Ok(()),
                handler(scope),
            )
            .expect("watch should initialize");

        owner.close().expect("owner disposal");
        assert_eq!(watcher.stop(), Ok(false));
    }
    root.close().expect("root disposal should succeed");
}

#[test]
fn ordinary_effects_can_be_stopped_through_the_same_handle() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0_i32).expect("fallible reactive creation");
            let runs_in_effect = runs.clone();
            let effect = scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get().expect("reactive read");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            assert_eq!(effect.stop(), Ok(true));
            source.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 1);
            assert_eq!(effect.stop(), Ok(false));
        })
        .expect("test operation should succeed");
}

#[test]
fn stopping_the_current_effect_does_not_write_back_deleted_metadata() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0_i32).expect("fallible reactive creation");
            let slot: Rc<Cell<Option<EffectHandle<'_>>>> = Rc::new(Cell::new(None));
            let slot_in_effect = slot.clone();
            let runs_in_effect = runs.clone();
            let effect = scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get().expect("reactive read");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        if let Some(effect) = slot_in_effect.get() {
                            effect.stop().expect("test operation should succeed");
                        }
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            slot.set(Some(effect));

            source.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
            source.set(2).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}
