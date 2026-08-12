use silex_reactivity::{ErrorHandler, Memo, Runtime, Scope, notify, track_batch};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn memo_and_derived_keep_their_notification_rules() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1i32).expect("fallible reactive creation");
            let memo_runs = Rc::new(Cell::new(0));
            let memo_runs_in_callback = memo_runs.clone();
            let memo_source = source;
            let memo = scope
                .memo(move |_| {
                    memo_runs_in_callback.set(memo_runs_in_callback.get() + 1);
                    memo_source.get().expect("reactive read") / 10
                })
                .expect("memo creation");
            let derived_runs = Rc::new(Cell::new(0));
            let derived_runs_in_callback = derived_runs.clone();
            let derived_source = source;
            let derived = scope
                .derived(
                    move || {
                        derived_runs_in_callback.set(derived_runs_in_callback.get() + 1);
                        Ok(derived_source.get().expect("reactive read") / 10)
                    },
                    handler(scope),
                )
                .expect("derived creation");

            assert_eq!(memo.get(), Ok(0));
            assert_eq!(derived.get(), Ok(0));
            set_source.set(2).expect("test operation should succeed");
            assert_eq!(memo.get(), Ok(0));
            assert_eq!(derived.get(), Ok(0));
            assert_eq!(memo_runs.get(), 2);
            assert_eq!(derived_runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn dependency_chain_evaluates_upstream_before_effect() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1i32).expect("fallible reactive creation");
            let middle_source = source;
            let middle = scope
                .memo(move |_| middle_source.get().expect("reactive read") + 1)
                .expect("memo creation");
            let tail_source = middle;
            let tail = scope
                .memo(move |_| tail_source.get().expect("reactive read") + 1)
                .expect("memo creation");
            let seen = Rc::new(Cell::new(0));
            let seen_in_effect = seen.clone();
            let tail_in_effect = tail;
            scope
                .effect(
                    move || {
                        seen_in_effect.set(tail_in_effect.get().expect("reactive read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(seen.get(), 3);
            set_source.set(4).expect("test operation should succeed");
            assert_eq!(seen.get(), 6);
        })
        .expect("test operation should succeed");
}

#[test]
fn diamond_dependencies_do_not_observe_intermediate_state() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(RefCell::new(Vec::new()));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1i32).expect("fallible reactive creation");
            let left = scope
                .memo(move |_| source.get().expect("reactive read") + 1)
                .expect("memo creation");
            let right = scope
                .memo(move |_| source.get().expect("reactive read") + 10)
                .expect("memo creation");
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    move || {
                        let left_value = left.get().expect("reactive read");
                        let right_value = right.get().expect("reactive read");
                        seen_in_effect.borrow_mut().push((
                            left_value,
                            right_value,
                            left_value + right_value,
                        ));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(seen.borrow().as_slice(), &[(2, 11, 13)]);
            set_source.set(2).expect("test operation should succeed");
            assert_eq!(seen.borrow().as_slice(), &[(2, 11, 13), (3, 12, 15)]);
        })
        .expect("test operation should succeed");
}

#[test]
fn dynamic_dependencies_are_replaced_on_each_effect_run() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (switch, set_switch) = scope.signal(true).expect("fallible reactive creation");
            let (left, set_left) = scope.signal(0i32).expect("fallible reactive creation");
            let (right, set_right) = scope.signal(0i32).expect("fallible reactive creation");
            let runs = Rc::new(Cell::new(0));
            let seen = Rc::new(Cell::new(0));
            let runs_in_effect = runs.clone();
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    move || {
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        seen_in_effect.set(if switch.get().expect("reactive read") {
                            left.get().expect("reactive read")
                        } else {
                            right.get().expect("reactive read")
                        });
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            set_right.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 1);
            set_left.set(2).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
            assert_eq!(seen.get(), 2);
            set_switch
                .set(false)
                .expect("test operation should succeed");
            assert_eq!(runs.get(), 3);
            assert_eq!(seen.get(), 1);
            set_left.set(3).expect("test operation should succeed");
            assert_eq!(runs.get(), 3);
            set_right.set(4).expect("test operation should succeed");
            assert_eq!(runs.get(), 4);
            assert_eq!(seen.get(), 4);
        })
        .expect("test operation should succeed");
}

#[test]
fn nested_memo_cleanup_does_not_track_the_outer_observer() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (outer_source, set_outer_source) =
                scope.signal(0i32).expect("fallible reactive creation");
            let (inner_source, set_inner_source) =
                scope.signal(0i32).expect("fallible reactive creation");
            let (probe, set_probe) = scope.signal(0i32).expect("fallible reactive creation");
            let cleanup_runs = Rc::new(Cell::new(0));
            let first_inner_run = Rc::new(Cell::new(true));
            let scope_for_cleanup = scope;
            let probe_for_cleanup = probe;
            let cleanup_runs_in_cleanup = cleanup_runs.clone();
            let cleanup_handler = handler(scope);
            let inner = scope
                .memo(move |_| {
                    let value = inner_source.get().expect("reactive read");
                    if first_inner_run.replace(false) {
                        let cleanup_runs_for_cleanup = cleanup_runs_in_cleanup.clone();
                        scope_for_cleanup
                            .on_cleanup(
                                move || {
                                    cleanup_runs_for_cleanup
                                        .set(cleanup_runs_for_cleanup.get() + 1);
                                    probe_for_cleanup
                                        .get()
                                        .expect("test operation should succeed");
                                    Ok(())
                                },
                                cleanup_handler,
                            )
                            .expect("cleanup should register");
                    }
                    value
                })
                .expect("memo creation");

            let outer_runs = Rc::new(Cell::new(0));
            let refresh_inner = Rc::new(Cell::new(false));
            let outer_inner = inner;
            let outer_source_in_effect = outer_source;
            let set_inner_source_in_effect = set_inner_source;
            let outer_runs_in_effect = outer_runs.clone();
            let refresh_inner_in_effect = refresh_inner.clone();
            scope
                .effect(
                    move || {
                        outer_source_in_effect
                            .get()
                            .expect("test operation should succeed");
                        outer_runs_in_effect.set(outer_runs_in_effect.get() + 1);
                        if refresh_inner_in_effect.replace(false) {
                            set_inner_source_in_effect
                                .set(1)
                                .expect("test operation should succeed");
                        }
                        outer_inner
                            .with_untracked(|_| ())
                            .expect("inner memo should remain readable");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(outer_runs.get(), 1);
            assert_eq!(cleanup_runs.get(), 0);

            refresh_inner.set(true);
            set_outer_source
                .set(1)
                .expect("test operation should succeed");

            assert_eq!(outer_runs.get(), 2);
            assert_eq!(cleanup_runs.get(), 1);

            set_probe.set(1).expect("test operation should succeed");
            assert_eq!(outer_runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn batch_delays_effects_and_untrack_preserves_ownership_context() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let (hidden, set_hidden) = scope.signal(0i32).expect("fallible reactive creation");
            let seen = Rc::new(Cell::new(0));
            let seen_in_effect = seen.clone();
            let effect_source = source;
            let effect_hidden = hidden;
            scope
                .effect(
                    move || {
                        seen_in_effect.set(
                            effect_source.get().expect("reactive read")
                                + effect_hidden.get().expect("reactive read"),
                        );
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            scope.batch(|| {
                set_source.set(1).expect("test operation should succeed");
                set_hidden.set(2).expect("test operation should succeed");
                assert_eq!(seen.get(), 0);
            });
            assert_eq!(seen.get(), 3);

            let tracked = Rc::new(Cell::new(0));
            let tracked_in_effect = tracked.clone();
            let second_source = source;
            let second_hidden = hidden;
            scope
                .effect(
                    move || {
                        tracked_in_effect.set(second_hidden.get().expect("reactive read"));
                        second_source.get().expect("test operation should succeed");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            set_hidden.set(4).expect("test operation should succeed");
            assert_eq!(tracked.get(), 4);
            assert_eq!(scope.untrack(|| hidden.get()), Ok(4));
            set_source.set(2).expect("test operation should succeed");
            assert_eq!(tracked.get(), 4);
        })
        .expect("test operation should succeed");
}

#[test]
fn epoch_memo_fast_path_skips_evaluation_when_upstream_unchanged() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(10i32).expect("fallible reactive creation");

            let m1_runs = Rc::new(Cell::new(0));
            let m1_runs_cb = m1_runs.clone();
            let m1 = scope
                .memo(move |_| {
                    m1_runs_cb.set(m1_runs_cb.get() + 1);
                    source.get().expect("reactive read") / 10
                })
                .expect("memo creation");

            let m2_runs = Rc::new(Cell::new(0));
            let m2_runs_cb = m2_runs.clone();
            let m2 = scope
                .memo(move |_| {
                    m2_runs_cb.set(m2_runs_cb.get() + 1);
                    m1.get().expect("reactive read") + 100
                })
                .expect("memo creation");

            let m3_runs = Rc::new(Cell::new(0));
            let m3_runs_cb = m3_runs.clone();
            let m3 = scope
                .memo(move |_| {
                    m3_runs_cb.set(m3_runs_cb.get() + 1);
                    m2.get().expect("reactive read") * 2
                })
                .expect("memo creation");

            assert_eq!(m3.get(), Ok(202));
            assert_eq!(m1_runs.get(), 1);
            assert_eq!(m2_runs.get(), 1);
            assert_eq!(m3_runs.get(), 1);

            set_source.set(15).expect("test operation should succeed");
            assert_eq!(m3.get(), Ok(202));
            assert_eq!(m1_runs.get(), 2);
            assert_eq!(m2_runs.get(), 1);
            assert_eq!(m3_runs.get(), 1);

            set_source.set(20).expect("test operation should succeed");
            assert_eq!(m3.get(), Ok(204));
            assert_eq!(m1_runs.get(), 3);
            assert_eq!(m2_runs.get(), 2);
            assert_eq!(m3_runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn track_batch_tracks_all_signals_in_one_scope() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (sig1, set_sig1) = scope.signal(10i32).expect("fallible reactive creation");
            let (sig2, set_sig2) = scope.signal(20i32).expect("fallible reactive creation");
            let runs_in_effect = runs.clone();

            scope
                .effect(
                    move || {
                        track_batch(&[sig1, sig2]).expect("test operation should succeed");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_sig1.set(11).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
            set_sig2.set(21).expect("test operation should succeed");
            assert_eq!(runs.get(), 3);
        })
        .expect("test operation should succeed");
}

#[test]
fn notify_recomputes_after_silent_interior_mutation() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope
                .signal(RefCell::new(0i32))
                .expect("fallible reactive creation");
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    move || {
                        seen_in_effect
                            .set(source.with(|value| *value.borrow()).expect("reactive read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            source
                .with(|value| {
                    *value.borrow_mut() = 1;
                    notify(&set_source).expect("test operation should succeed");
                    assert_eq!(seen.get(), 0);
                })
                .expect("test operation should succeed");

            assert_eq!(seen.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn cross_scope_silent_notify_from_callback_waits_for_value_borrow() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope
                .signal(RefCell::new(0i32))
                .expect("fallible reactive creation");
            scope
                .child(|child| {
                    let runs_in_effect = runs.clone();
                    child
                        .effect(
                            move || {
                                source
                                    .with(|value| {
                                        std::hint::black_box(*value.borrow());
                                        runs_in_effect.set(runs_in_effect.get() + 1);
                                    })
                                    .expect("test operation should succeed");
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("effect should initialize");

                    let runs_in_callback = runs.clone();
                    let callback = child
                        .callback(move |_: ()| {
                            let runs_before = runs_in_callback.get();
                            source
                                .with(|value| {
                                    *value.borrow_mut() += 1;
                                    notify(&set_source).expect("test operation should succeed");
                                    assert_eq!(runs_in_callback.get(), runs_before);
                                })
                                .expect("test operation should succeed");
                            Ok::<(), ()>(())
                        })
                        .expect("callback should initialize");

                    assert_eq!(runs.get(), 1);
                    callback.invoke(()).expect("callback should be alive");
                    assert_eq!(runs.get(), 2);
                    assert_eq!(source.with(|value| *value.borrow()), Ok(1));

                    callback
                        .invoke(())
                        .expect("callback should remain reusable");
                    assert_eq!(runs.get(), 3);
                    assert_eq!(source.with(|value| *value.borrow()), Ok(2));
                })
                .expect("test operation should succeed");
        })
        .expect("test operation should succeed");
}

#[test]
fn cross_scope_computation_stack_includes_scope_identity() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1i32).expect("fallible reactive creation");
            let parent_memo = scope
                .memo(move |_| source.get().expect("reactive read") + 1)
                .expect("memo creation");

            scope
                .child(|child| {
                    let parent_memo_in_child = parent_memo;
                    let child_memo = child
                        .memo(move |_| parent_memo_in_child.get().expect("reactive read") + 1)
                        .expect("memo creation");
                    let seen = Rc::new(Cell::new(0));
                    let seen_in_effect = seen.clone();
                    child
                        .effect(
                            move || {
                                seen_in_effect.set(child_memo.get().expect("reactive read"));
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("effect should initialize");

                    assert_eq!(seen.get(), 3);
                    set_source.set(2).expect("test operation should succeed");
                    assert_eq!(seen.get(), 4);
                })
                .expect("test operation should succeed");
        })
        .expect("test operation should succeed");
}

#[test]
fn cross_scope_derived_reacts_and_detaches_on_exit() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1i32).expect("fallible reactive creation");
            scope
                .child(|child| {
                    let child_source = source;
                    let derived = child
                        .derived(
                            move || Ok(child_source.get().expect("reactive read") * 2),
                            handler(child),
                        )
                        .expect("derived creation");
                    let seen_in_effect = seen.clone();
                    child
                        .effect(
                            move || {
                                seen_in_effect.set(derived.get().expect("reactive read"));
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("effect should initialize");

                    assert_eq!(seen.get(), 2);
                    set_source.set(2).expect("test operation should succeed");
                    assert_eq!(seen.get(), 4);
                })
                .expect("test operation should succeed");

            set_source.set(3).expect("test operation should succeed");
            assert_eq!(seen.get(), 4);
        })
        .expect("test operation should succeed");
}

#[test]
fn cyclic_memo_dependency_panics_without_poisoning_the_scheduler() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let first_slot: Rc<RefCell<Option<Memo<'_, i32>>>> = Rc::new(RefCell::new(None));
            let second_slot: Rc<RefCell<Option<Memo<'_, i32>>>> = Rc::new(RefCell::new(None));
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");

            let second_slot_in_first = second_slot.clone();
            let first = scope
                .memo(move |_| {
                    let dependency = second_slot_in_first.borrow().as_ref().copied();
                    source.get().expect("reactive read")
                        + dependency
                            .map(|memo| memo.get().expect("reactive read"))
                            .unwrap_or(0)
                })
                .expect("memo creation");
            *first_slot.borrow_mut() = Some(first);

            let first_slot_in_second = first_slot.clone();
            let second = scope
                .memo(move |_| {
                    let dependency = first_slot_in_second.borrow().as_ref().copied();
                    source.get().expect("reactive read")
                        + dependency
                            .map(|memo| memo.get().expect("reactive read"))
                            .unwrap_or(0)
                })
                .expect("memo creation");
            *second_slot.borrow_mut() = Some(second);

            set_source.set(1).expect("test operation should succeed");
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                first.get().expect("test operation should succeed");
            }));
            assert!(panic.is_err());

            set_source.set(2).expect("test operation should succeed");
            assert_eq!(source.get(), Ok(2));
        })
        .expect("test operation should succeed");
}

#[test]
fn cyclic_effect_queue_failure_does_not_poison_unrelated_effects() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let first_slot: Rc<RefCell<Option<Memo<'_, i32>>>> = Rc::new(RefCell::new(None));
            let second_slot: Rc<RefCell<Option<Memo<'_, i32>>>> = Rc::new(RefCell::new(None));
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let (refresh, set_refresh) = scope.signal(0i32).expect("fallible reactive creation");

            let second_slot_in_first = second_slot.clone();
            let first = scope
                .memo(move |_| {
                    refresh.get().expect("test operation should succeed");
                    let dependency = second_slot_in_first.borrow().as_ref().copied();
                    source.get().expect("reactive read")
                        + dependency
                            .map(|memo| memo.get().expect("reactive read"))
                            .unwrap_or(0)
                })
                .expect("memo creation");
            *first_slot.borrow_mut() = Some(first);

            let first_slot_in_second = first_slot.clone();
            let second = scope
                .memo(move |_| {
                    let dependency = first_slot_in_second.borrow().as_ref().copied();
                    source.get().expect("reactive read")
                        + dependency
                            .map(|memo| memo.get().expect("reactive read"))
                            .unwrap_or(0)
                })
                .expect("memo creation");
            *second_slot.borrow_mut() = Some(second);

            set_refresh.set(1).expect("test operation should succeed");
            assert_eq!(first.get(), Ok(0));

            let first_in_effect = first;
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        first_in_effect
                            .get()
                            .expect("test operation should succeed");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                set_source.set(1).expect("source update should succeed");
            }));
            assert!(panic.is_err());

            let independent_runs = Rc::new(Cell::new(0));
            let (independent, set_independent) =
                scope.signal(0i32).expect("fallible reactive creation");
            let independent_runs_in_effect = independent_runs.clone();
            scope
                .effect(
                    move || {
                        independent_runs_in_effect.set(
                            independent_runs_in_effect.get()
                                + independent.get().expect("reactive read"),
                        );
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            set_independent
                .set(1)
                .expect("test operation should succeed");
            assert_eq!(independent_runs.get(), 1);
            assert_eq!(source.get(), Ok(1));
        })
        .expect("test operation should succeed");
}
