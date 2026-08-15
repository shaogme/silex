use silex_reactivity::{
    CallbackInvokeError, ErrorHandlerToken, Memo, ReactiveError, Runtime, Scope,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn panic_in_update_keeps_the_value_and_releases_the_lease() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (signal, set_signal) = scope.signal(1i32).expect("fallible reactive creation");
            let panic = catch_unwind(AssertUnwindSafe(|| {
                set_signal
                    .update(|_| panic!("update panic"))
                    .expect("test operation should succeed");
            }));
            assert!(panic.is_err());
            assert_eq!(signal.get(), Ok(1));
        })
        .expect("test operation should succeed");

    runtime
        .child(|scope| {
            let (signal, set_signal) = scope.signal(1i32).expect("fallible reactive creation");
            set_signal.set(2).expect("test operation should succeed");
            assert_eq!(signal.get(), Ok(2));
        })
        .expect("test operation should succeed");
}

#[test]
fn shared_reads_succeed_but_write_conflicts_are_reported() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (signal, set_signal) = scope.signal(1i32).expect("fallible reactive creation");

            let nested_read = signal
                .with(|_| signal.get())
                .expect("shared reads should be nestable");
            assert_eq!(nested_read, Ok(1));

            let read_then_write = signal
                .with(|_| set_signal.set(2))
                .expect("read lease should remain observable");
            assert_eq!(read_then_write, Err(ReactiveError::BorrowConflict));

            let write_then_read = set_signal
                .update(|_| signal.get())
                .expect("write lease should remain observable");
            assert_eq!(write_then_read, Err(ReactiveError::BorrowConflict));

            let write_then_write = set_signal
                .update(|_| set_signal.set(2))
                .expect("write lease should remain observable");
            assert_eq!(write_then_write, Err(ReactiveError::BorrowConflict));
            assert_eq!(signal.get(), Ok(1));
        })
        .expect("test operation should succeed");
}

#[test]
fn recursive_memo_read_reports_reentrant_instead_of_borrow_conflict() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let slot: Rc<Cell<Option<Memo<'_, i32, ()>>>> = Rc::new(Cell::new(None));
            let slot_in_memo = slot.clone();
            let (source, set_source) = scope.signal(1_i32).expect("fallible reactive creation");
            let memo = scope
                .memo(
                    move |_| {
                        let value = source.get().expect("reactive read");
                        if let Some(memo) = slot_in_memo.get() {
                            assert_eq!(
                                memo.get(),
                                Err(CallbackInvokeError::Runtime(ReactiveError::Reentrant))
                            );
                        }
                        Ok(value)
                    },
                    handler(scope),
                )
                .expect("memo creation");
            slot.set(Some(memo));

            set_source.set(2).expect("test operation should succeed");
            assert_eq!(memo.get(), Ok(2));
        })
        .expect("test operation should succeed");
}

#[test]
fn panic_in_effect_does_not_block_the_next_notification() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    let should_panic = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let runs_in_effect = runs.clone();
            let panic_in_effect = should_panic.clone();
            let _effect = scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
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
                    .set(1)
                    .expect("first effect notification should execute");
            }));
            assert!(panic.is_err());
            assert_eq!(runs.get(), 2);

            set_source
                .set(2)
                .expect("effect should be schedulable after a panic");
            assert_eq!(runs.get(), 3);
        })
        .expect("test operation should succeed");
}

#[test]
fn cleanup_panic_during_effect_rerun_does_not_skip_remaining_cleanups() {
    let mut runtime = Runtime::new();
    let remaining_cleanup_ran = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let scope_copy = scope;
            let register_cleanups = Rc::new(Cell::new(true));
            let effect_runs = Rc::new(Cell::new(0));
            let effect_runs_in_effect = effect_runs.clone();
            let remaining_cleanup_ran_in_effect = remaining_cleanup_ran.clone();
            scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
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

            set_source.set(2).expect("test operation should succeed");
            assert_eq!(effect_runs.get(), 2);

            let (independent, set_independent) =
                scope.signal(0i32).expect("fallible reactive creation");
            let seen = Rc::new(Cell::new(0));
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    move || {
                        seen_in_effect.set(independent.get().expect("reactive read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            set_independent
                .set(1)
                .expect("test operation should succeed");
            assert_eq!(seen.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn panic_in_memo_keeps_the_previous_value_and_allows_retry() {
    let mut runtime = Runtime::new();
    let should_panic = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1i32).expect("fallible reactive creation");
            let panic_in_memo = should_panic.clone();
            let memo = scope
                .memo(
                    move |_| {
                        let value = source.get().expect("reactive read");
                        if panic_in_memo.replace(false) {
                            panic!("memo panic");
                        }
                        Ok(value * 2)
                    },
                    handler(scope),
                )
                .expect("memo creation");

            assert_eq!(memo.get(), Ok(2));
            should_panic.set(true);
            set_source.set(2).expect("test operation should succeed");
            let panic = catch_unwind(AssertUnwindSafe(|| {
                memo.get().expect("test operation should succeed");
            }));
            assert!(panic.is_err());

            assert_eq!(memo.get(), Ok(4));
        })
        .expect("test operation should succeed");
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

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1i32).expect("fallible reactive creation");
            let panic_in_eq = should_panic.clone();
            let memo = scope
                .memo(
                    move |_| {
                        Ok(PanicOnCompare {
                            value: source.get().expect("reactive read"),
                            should_panic: panic_in_eq.clone(),
                        })
                    },
                    handler(scope),
                )
                .expect("memo creation");

            assert_eq!(memo.get().expect("reactive read").value, 1);
            should_panic.set(true);
            set_source.set(2).expect("test operation should succeed");

            let panic = catch_unwind(AssertUnwindSafe(|| {
                memo.get().expect("test operation should succeed");
            }));
            assert!(panic.is_err());
            assert_eq!(memo.get().expect("reactive read").value, 2);

            set_source.set(3).expect("test operation should succeed");
            assert_eq!(memo.get().expect("reactive read").value, 3);
        })
        .expect("test operation should succeed");
}

#[test]
fn batch_panic_restores_depth_and_flushes_pending_effects() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    move || {
                        seen_in_effect.set(source.get().expect("reactive read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            let panic = catch_unwind(AssertUnwindSafe(|| {
                let _ = scope.batch(|| {
                    set_source.set(1).expect("test operation should succeed");
                    panic!("batch panic");
                });
            }));
            assert!(panic.is_err());
            assert_eq!(seen.get(), 1);

            set_source.set(2).expect("test operation should succeed");
            assert_eq!(seen.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn untrack_panic_restores_the_active_dependency_observer() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    let first_run = Rc::new(Cell::new(true));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let (tracked, set_tracked) = scope.signal(0i32).expect("fallible reactive creation");
            let scope_copy = scope;
            let runs_in_effect = runs.clone();
            let first_run_in_effect = first_run.clone();
            scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
                        if first_run_in_effect.replace(false) {
                            let panic = catch_unwind(AssertUnwindSafe(|| {
                                scope_copy.untrack(|| panic!("untrack panic"));
                            }));
                            assert!(panic.is_err());
                        }
                        tracked.get().expect("test operation should succeed");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_tracked.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
            set_source.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 3);
        })
        .expect("test operation should succeed");
}

#[test]
fn child_callback_panic_restores_the_outer_observer_frame() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let parent_scope = scope;
            let (source, _) = scope.signal(0i32).expect("fallible reactive creation");
            let (tail, set_tail) = scope.signal(0i32).expect("fallible reactive creation");
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
                        let panic = catch_unwind(AssertUnwindSafe(|| {
                            parent_scope
                                .child(|_| panic!("child callback panic"))
                                .expect("test operation should succeed");
                        }));
                        assert!(panic.is_err());
                        tail.get().expect("test operation should succeed");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_tail.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}
