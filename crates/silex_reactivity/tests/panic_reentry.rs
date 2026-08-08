use silex_reactivity::{ErrorHandler, Memo, ReactiveError, Runtime, Scope};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {})
}

#[test]
fn panic_in_update_keeps_the_value_and_releases_the_lease() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (signal, set_signal) = scope.signal(1i32);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            set_signal.update(|_| panic!("update panic"));
        }));
        assert!(panic.is_err());
        assert_eq!(signal.get(), 1);
    });

    runtime.child(|scope| {
        let (signal, set_signal) = scope.signal(1i32);
        set_signal.set(2);
        assert_eq!(signal.get(), 2);
    });
}

#[test]
fn shared_reads_succeed_but_write_conflicts_are_reported() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (signal, set_signal) = scope.signal(1i32);

        let nested_read = signal
            .try_with(|_| signal.try_get())
            .expect("shared reads should be nestable");
        assert_eq!(nested_read, Ok(1));

        let read_then_write = signal
            .try_with(|_| set_signal.try_set(2))
            .expect("read lease should remain observable");
        assert_eq!(read_then_write, Err(ReactiveError::BorrowConflict));

        let write_then_read = set_signal
            .try_update(|_| signal.try_get())
            .expect("write lease should remain observable");
        assert_eq!(write_then_read, Err(ReactiveError::BorrowConflict));

        let write_then_write = set_signal
            .try_update(|_| set_signal.try_set(2))
            .expect("write lease should remain observable");
        assert_eq!(write_then_write, Err(ReactiveError::BorrowConflict));
        assert_eq!(signal.get(), 1);
    });
}

#[test]
fn recursive_memo_read_reports_reentrant_instead_of_borrow_conflict() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let slot: Rc<Cell<Option<Memo<'_, i32>>>> = Rc::new(Cell::new(None));
        let slot_in_memo = slot.clone();
        let (source, set_source) = scope.signal(1_i32);
        let memo = scope.memo(move |_| {
            let value = source.get();
            if let Some(memo) = slot_in_memo.get() {
                assert_eq!(memo.try_get(), Err(ReactiveError::Reentrant));
            }
            value
        });
        slot.set(Some(memo));

        set_source.set(2);
        assert_eq!(memo.get(), 2);
    });
}

#[test]
fn panic_in_effect_does_not_block_the_next_notification() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    let should_panic = Rc::new(Cell::new(false));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let runs_in_effect = runs.clone();
        let panic_in_effect = should_panic.clone();
        let _effect = scope
            .effect(
                move || {
                    let _ = source.get();
                    runs_in_effect.set(runs_in_effect.get() + 1);
                    if panic_in_effect.replace(false) {
                        panic!("effect panic");
                    }
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");

        should_panic.set(true);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            set_source
                .try_set(1)
                .expect("first effect notification should execute");
        }));
        assert!(panic.is_err());
        assert_eq!(runs.get(), 2);

        set_source
            .try_set(2)
            .expect("effect should be schedulable after a panic");
        assert_eq!(runs.get(), 3);
    });
}

#[test]
fn cleanup_panic_during_effect_rerun_does_not_skip_remaining_cleanups() {
    let mut runtime = Runtime::new();
    let remaining_cleanup_ran = Rc::new(Cell::new(false));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let scope_copy = scope;
        let register_cleanups = Rc::new(Cell::new(true));
        let effect_runs = Rc::new(Cell::new(0));
        let effect_runs_in_effect = effect_runs.clone();
        let remaining_cleanup_ran_in_effect = remaining_cleanup_ran.clone();
        scope
            .effect(
                move || {
                    let _ = source.get();
                    effect_runs_in_effect.set(effect_runs_in_effect.get() + 1);
                    if register_cleanups.replace(false) {
                        scope_copy
                            .on_cleanup(|| panic!("effect cleanup panic"), handler(scope_copy))
                            .expect("cleanup should register");
                        let remaining_cleanup_ran = remaining_cleanup_ran_in_effect.clone();
                        scope_copy
                            .on_cleanup(
                                move || {
                                    remaining_cleanup_ran.set(true);
                                    Ok(())
                                },
                                handler(scope_copy),
                            )
                            .expect("cleanup should register");
                    }
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");

        let panic = catch_unwind(AssertUnwindSafe(|| set_source.set(1)));
        assert!(panic.is_err());
        assert!(remaining_cleanup_ran.get());
        assert_eq!(effect_runs.get(), 1);

        set_source.set(2);
        assert_eq!(effect_runs.get(), 2);

        let (independent, set_independent) = scope.signal(0i32);
        let seen = Rc::new(Cell::new(0));
        let seen_in_effect = seen.clone();
        scope
            .effect(
                move || {
                    seen_in_effect.set(independent.get());
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");
        set_independent.set(1);
        assert_eq!(seen.get(), 1);
    });
}

#[test]
fn panic_in_memo_keeps_the_previous_value_and_allows_retry() {
    let mut runtime = Runtime::new();
    let should_panic = Rc::new(Cell::new(false));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let panic_in_memo = should_panic.clone();
        let memo = scope.memo(move |_| {
            let value = source.get();
            if panic_in_memo.replace(false) {
                panic!("memo panic");
            }
            value * 2
        });

        assert_eq!(memo.get(), 2);
        should_panic.set(true);
        set_source.set(2);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = memo.get();
        }));
        assert!(panic.is_err());

        assert_eq!(memo.get(), 4);
    });
}

#[test]
fn panic_in_memo_equality_keeps_the_previous_value_and_allows_retry() {
    #[derive(Clone)]
    struct PanicOnCompare {
        value: i32,
        should_panic: Rc<Cell<bool>>,
    }

    impl PartialEq for PanicOnCompare {
        fn eq(&self, other: &Self) -> bool {
            if self.should_panic.replace(false) {
                panic!("memo equality panic");
            }
            self.value == other.value
        }
    }

    let mut runtime = Runtime::new();
    let should_panic = Rc::new(Cell::new(false));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let panic_in_eq = should_panic.clone();
        let memo = scope.memo(move |_| PanicOnCompare {
            value: source.get(),
            should_panic: panic_in_eq.clone(),
        });

        assert_eq!(memo.get().value, 1);
        should_panic.set(true);
        set_source.set(2);

        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = memo.get();
        }));
        assert!(panic.is_err());
        assert_eq!(memo.get().value, 2);

        set_source.set(3);
        assert_eq!(memo.get().value, 3);
    });
}

#[test]
fn batch_panic_restores_depth_and_flushes_pending_effects() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let seen_in_effect = seen.clone();
        scope
            .effect(
                move || {
                    seen_in_effect.set(source.get());
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");

        let panic = catch_unwind(AssertUnwindSafe(|| {
            scope.batch(|| {
                set_source.set(1);
                panic!("batch panic");
            });
        }));
        assert!(panic.is_err());
        assert_eq!(seen.get(), 1);

        set_source.set(2);
        assert_eq!(seen.get(), 2);
    });
}

#[test]
fn untrack_panic_restores_the_active_dependency_observer() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    let first_run = Rc::new(Cell::new(true));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let (tracked, set_tracked) = scope.signal(0i32);
        let scope_copy = scope;
        let runs_in_effect = runs.clone();
        let first_run_in_effect = first_run.clone();
        scope
            .effect(
                move || {
                    let _ = source.get();
                    if first_run_in_effect.replace(false) {
                        let panic = catch_unwind(AssertUnwindSafe(|| {
                            scope_copy.untrack(|| panic!("untrack panic"));
                        }));
                        assert!(panic.is_err());
                    }
                    let _ = tracked.get();
                    runs_in_effect.set(runs_in_effect.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");

        assert_eq!(runs.get(), 1);
        set_tracked.set(1);
        assert_eq!(runs.get(), 2);
        set_source.set(1);
        assert_eq!(runs.get(), 3);
    });
}

#[test]
fn child_callback_panic_restores_the_outer_observer_frame() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let parent_scope = scope;
        let (source, _) = scope.signal(0i32);
        let (tail, set_tail) = scope.signal(0i32);
        let runs_in_effect = runs.clone();
        scope
            .effect(
                move || {
                    let _ = source.get();
                    let panic = catch_unwind(AssertUnwindSafe(|| {
                        parent_scope.child(|_| panic!("child callback panic"));
                    }));
                    assert!(panic.is_err());
                    let _ = tail.get();
                    runs_in_effect.set(runs_in_effect.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");

        assert_eq!(runs.get(), 1);
        set_tail.set(1);
        assert_eq!(runs.get(), 2);
    });
}
