use silex_reactivity::{Effect, EffectInitError, ErrorHandler, Runtime, Scope, WatchOptions};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {})
}

#[test]
fn getter_watch_commits_values_and_gates_equal_updates() {
    let mut runtime = Runtime::new();
    let getter_runs = Rc::new(Cell::new(0));
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1_i32);
        let getter_runs_in_getter = getter_runs.clone();
        let calls_in_callback = calls.clone();
        scope
            .watch_getter(
                move || {
                    getter_runs_in_getter.set(getter_runs_in_getter.get() + 1);
                    Ok(source.get())
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
        set_source.set(1);
        assert_eq!(getter_runs.get(), 2);
        assert!(calls.borrow().is_empty());
        set_source.set(2);
        assert_eq!(calls.borrow().as_slice(), &[(2, Some(1))]);
    });
}

#[test]
fn immediate_once_watch_stops_after_the_initial_callback() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1_i32);
        let calls_in_callback = calls.clone();
        let watcher = scope
            .watch_getter_with_options(
                move || Ok(source.get()),
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
        assert_eq!(watcher.try_stop(), Ok(false));
        set_source.set(2);
        assert_eq!(calls.get(), 1);
    });
}

#[test]
fn callback_reads_are_untracked_and_dynamic_getter_dependencies_replace() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (switch, set_switch) = scope.signal(true);
        let (left, set_left) = scope.signal(1_i32);
        let (right, set_right) = scope.signal(10_i32);
        let (probe, set_probe) = scope.signal(0_i32);
        let calls_in_callback = calls.clone();
        scope
            .watch_getter(
                move || {
                    if switch.get() {
                        Ok(left.get())
                    } else {
                        Ok(right.get())
                    }
                },
                move |_, _| {
                    let _ = probe.get();
                    calls_in_callback.set(calls_in_callback.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("watch should initialize");

        set_probe.set(1);
        assert_eq!(calls.get(), 0);
        set_right.set(11);
        assert_eq!(calls.get(), 0);
        set_left.set(2);
        assert_eq!(calls.get(), 1);
        set_switch.set(false);
        assert_eq!(calls.get(), 2);
        set_left.set(3);
        assert_eq!(calls.get(), 2);
        set_right.set(12);
        assert_eq!(calls.get(), 3);
    });
}

#[test]
fn stop_cancels_future_runs_and_runs_cleanup_once() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(Cell::new(0));
    let cleanups = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0_i32);
        let calls_in_callback = calls.clone();
        let cleanups_in_callback = cleanups.clone();
        let watcher_scope = scope;
        let watcher = scope
            .watch_getter(
                move || Ok(source.get()),
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

        set_source.set(1);
        assert_eq!(calls.get(), 1);
        set_source.set(2);
        assert_eq!(cleanups.get(), 1);
        assert_eq!(watcher.try_stop(), Ok(true));
        assert_eq!(cleanups.get(), 2);
        assert_eq!(watcher.try_stop(), Ok(false));
        set_source.set(3);
        assert_eq!(calls.get(), 2);
    });

    assert_eq!(cleanups.get(), 2);
}

#[test]
fn callback_panic_keeps_the_old_snapshot_for_a_later_retry() {
    let mut runtime = Runtime::new();
    let should_panic = Rc::new(Cell::new(true));
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0_i32);
        let should_panic_in_callback = should_panic.clone();
        let calls_in_callback = calls.clone();
        scope
            .watch_getter(
                move || Ok(source.get()),
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

        let panic = catch_unwind(AssertUnwindSafe(|| set_source.set(1)));
        assert!(panic.is_err());
        set_source.set(2);
        assert_eq!(calls.borrow().as_slice(), &[(1, Some(0)), (2, Some(0))]);
    });
}

#[test]
fn initial_watch_panic_rolls_back_the_registered_node() {
    let mut runtime = Runtime::new();
    let getter_runs = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0_i32);
        let getter_runs_in_getter = getter_runs.clone();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = scope.watch_getter(
                move || {
                    getter_runs_in_getter.set(getter_runs_in_getter.get() + 1);
                    let value = source.get();
                    if value == 0 {
                        panic!("initial watch panic");
                    }
                    Ok(value)
                },
                |_, _| Ok(()),
                handler(scope),
            );
        }));
        assert!(panic.is_err());
        set_source.set(1);
        assert_eq!(getter_runs.get(), 1);
    });
}

#[test]
fn foreign_watch_inputs_fail_before_getter_or_callback_execution() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    let foreign_inputs = first.child(|scope| {
        let (source, _) = scope.signal(1_i32);
        silex_reactivity::RuntimeInputs::single(source.runtime_input())
    });
    let getter_runs = Rc::new(Cell::new(0));
    let callback_runs = Rc::new(Cell::new(0));

    let result = second.child(|scope| {
        let getter_runs_in_getter = getter_runs.clone();
        let callback_runs_in_callback = callback_runs.clone();
        scope
            .watch_getter_from(
                foreign_inputs,
                move || {
                    getter_runs_in_getter.set(getter_runs_in_getter.get() + 1);
                    Ok(1_i32)
                },
                move |_, _| {
                    callback_runs_in_callback.set(callback_runs_in_callback.get() + 1);
                    Ok(())
                },
                handler(scope),
                WatchOptions::default(),
            )
            .map(|_| ())
    });

    assert!(matches!(
        result,
        Err(EffectInitError::Registration(
            silex_reactivity::ReactiveError::RuntimeMismatch,
        ))
    ));
    assert_eq!(getter_runs.get(), 0);
    assert_eq!(callback_runs.get(), 0);
}

#[test]
fn owner_disposal_makes_a_watcher_handle_stopped() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        let owner = scope.owned_scope();
        let watcher = owner
            .watch_getter(|| Ok(1_i32), |_, _| Ok(()), handler(scope))
            .expect("watch should initialize");

        owner.dispose();
        assert_eq!(watcher.try_stop(), Ok(false));
    }
    root.dispose().expect("root disposal should succeed");
}

#[test]
fn ordinary_effects_can_be_stopped_through_the_same_handle() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0_i32);
        let runs_in_effect = runs.clone();
        let effect = scope
            .effect(
                move || {
                    let _ = source.get();
                    runs_in_effect.set(runs_in_effect.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");

        assert_eq!(runs.get(), 1);
        assert_eq!(effect.try_stop(), Ok(true));
        set_source.set(1);
        assert_eq!(runs.get(), 1);
        assert_eq!(effect.try_stop(), Ok(false));
    });
}

#[test]
fn stopping_the_current_effect_does_not_write_back_deleted_metadata() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0_i32);
        let slot: Rc<Cell<Option<Effect<'_>>>> = Rc::new(Cell::new(None));
        let slot_in_effect = slot.clone();
        let runs_in_effect = runs.clone();
        let effect = scope
            .effect(
                move || {
                    let _ = source.get();
                    runs_in_effect.set(runs_in_effect.get() + 1);
                    if let Some(effect) = slot_in_effect.get() {
                        effect.stop();
                    }
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");
        slot.set(Some(effect));

        set_source.set(1);
        assert_eq!(runs.get(), 2);
        set_source.set(2);
        assert_eq!(runs.get(), 2);
    });
}
