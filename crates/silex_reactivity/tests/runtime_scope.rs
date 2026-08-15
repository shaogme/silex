use silex_reactivity::{
    CallbackInvokeError, ErrorHandlerToken, ReactiveError, ReadSignal, Runtime, Scope, notify,
    unwind_safe,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

struct CleanupStoredProbe {
    value: Rc<Cell<i32>>,
    drops: Rc<Cell<usize>>,
    dropped_value: Rc<Cell<i32>>,
}

impl Drop for CleanupStoredProbe {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        self.dropped_value.set(self.value.get());
    }
}

struct TrackDuringDrop<'scope> {
    source: ReadSignal<'scope, i32>,
    tracked: Rc<Cell<bool>>,
}

impl Drop for TrackDuringDrop<'_> {
    fn drop(&mut self) {
        self.tracked.set(self.source.get().is_ok());
    }
}

#[test]
fn runtime_run_provides_scoped_signal_and_effect() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (count, set_count) = scope.signal(0i32).expect("fallible reactive creation");
            let doubled = scope
                .memo(
                    move |_| Ok(count.get().expect("reactive read") * 2),
                    handler(scope),
                )
                .expect("memo creation");
            let runs_in_effect = runs.clone();
            let doubled_in_effect = doubled;
            let _effect = scope
                .effect(
                    move || {
                        doubled_in_effect
                            .get()
                            .expect("test operation should succeed");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_count.set(1).expect("test operation should succeed");
            assert_eq!(doubled.get(), Ok(2));
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");

    assert_eq!(runs.get(), 2);
}

#[test]
fn non_static_effect_can_capture_data_and_scoped_signal() {
    let mut runtime = Runtime::new();
    let external = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (signal, set_signal) = scope.signal(1i32).expect("fallible reactive creation");
            let external_in_effect = external.clone();
            scope
                .effect(
                    move || {
                        external_in_effect
                            .set(external_in_effect.get() + signal.get().expect("reactive read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            set_signal.set(2).expect("test operation should succeed");
        })
        .expect("test operation should succeed");

    assert_eq!(external.get(), 3);
}

#[test]
fn child_scope_is_lexical_and_cleans_up_its_nodes() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            scope
                .child(|child| {
                    let (local, set_local) =
                        child.signal(0i32).expect("fallible reactive creation");
                    let runs = cleaned.clone();
                    let _effect = child
                        .effect(
                            move || {
                                local.get().expect("test operation should succeed");
                                runs.set(runs.get() + 1);
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("effect should initialize");
                    set_local.set(1).expect("test operation should succeed");
                    assert_eq!(cleaned.get(), 2);
                })
                .expect("test operation should succeed");
            assert_eq!(cleaned.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn child_effect_reacts_to_parent_signal_and_detaches_on_exit() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (parent, set_parent) = scope.signal(0i32).expect("fallible reactive creation");
            scope
                .child(|child| {
                    let runs_in_effect = runs.clone();
                    child
                        .effect(
                            move || {
                                parent.get().expect("test operation should succeed");
                                runs_in_effect.set(runs_in_effect.get() + 1);
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("effect should initialize");
                    assert_eq!(runs.get(), 1);
                    set_parent.set(1).expect("test operation should succeed");
                    assert_eq!(runs.get(), 2);
                })
                .expect("test operation should succeed");

            set_parent.set(2).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn child_cleanup_runs_when_scoped_run_ends() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(false));
    let cleaned_in_scope = cleaned.clone();
    runtime
        .child(|scope| {
            scope
                .on_cleanup(
                    move || {
                        cleaned_in_scope.set(true);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
            assert!(!cleaned.get());
        })
        .expect("test operation should succeed");
    assert!(cleaned.get());
}

#[test]
fn final_cleanup_updates_stored_value_before_payload_drop() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(RefCell::new(Vec::new()));
    let drops = Rc::new(Cell::new(0));
    let dropped_value = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let value = Rc::new(Cell::new(1));
            let stored = scope
                .stored(CleanupStoredProbe {
                    value: value.clone(),
                    drops: drops.clone(),
                    dropped_value: dropped_value.clone(),
                })
                .expect("stored creation");
            let observed_in_cleanup = observed.clone();
            let scope_in_cleanup = scope;
            scope
                .on_cleanup(
                    move || {
                        assert!(!scope_in_cleanup.is_active());
                        observed_in_cleanup
                            .borrow_mut()
                            .push(stored.with(|probe| probe.value.get()).expect("stored read"));
                        stored
                            .update(|probe| probe.value.set(2))
                            .expect("stored update");
                        observed_in_cleanup
                            .borrow_mut()
                            .push(stored.with(|probe| probe.value.get()).expect("stored read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    assert_eq!(observed.borrow().as_slice(), &[1, 2]);
    assert_eq!(drops.get(), 1);
    assert_eq!(dropped_value.get(), 2);
}

#[test]
fn payload_drop_cannot_track_through_either_observer_slot() {
    let mut runtime = Runtime::new();
    let tracked = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, _) = scope.signal(0_i32).expect("source creation");
            scope
                .child(|child| {
                    child
                        .stored(TrackDuringDrop {
                            source,
                            tracked: tracked.clone(),
                        })
                        .expect("stored creation");
                })
                .expect("child scope should dispose its payload");
        })
        .expect("test operation should succeed");

    assert!(tracked.get());
}

#[test]
fn computation_cleanup_can_access_its_child_stored_value_before_root_cleanup() {
    let mut runtime = Runtime::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let observed_value = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let dropped_value = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let scope_in_effect = scope;
            let events_in_effect = events.clone();
            let observed_value_in_effect = observed_value.clone();
            let drops_in_effect = drops.clone();
            let dropped_value_in_effect = dropped_value.clone();
            scope
                .effect(
                    move || {
                        let value = Rc::new(Cell::new(3));
                        let stored = scope_in_effect
                            .stored(CleanupStoredProbe {
                                value: value.clone(),
                                drops: drops_in_effect.clone(),
                                dropped_value: dropped_value_in_effect.clone(),
                            })
                            .expect("stored creation");
                        let events_in_cleanup = events_in_effect.clone();
                        let observed_value_in_cleanup = observed_value_in_effect.clone();
                        scope_in_effect
                            .on_cleanup(
                                move || {
                                    stored
                                        .update(|probe| probe.value.set(4))
                                        .expect("stored update");
                                    observed_value_in_cleanup.set(
                                        stored
                                            .with(|probe| probe.value.get())
                                            .expect("stored read"),
                                    );
                                    events_in_cleanup.borrow_mut().push("node");
                                    Ok(())
                                },
                                handler(scope_in_effect),
                            )
                            .expect("effect cleanup should register");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            let events_in_root_cleanup = events.clone();
            scope
                .on_cleanup(
                    move || {
                        events_in_root_cleanup.borrow_mut().push("root");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("root cleanup should register");
        })
        .expect("test operation should succeed");

    assert_eq!(events.borrow().as_slice(), &["node", "root"]);
    assert_eq!(observed_value.get(), 4);
    assert_eq!(drops.get(), 1);
    assert_eq!(dropped_value.get(), 4);
}

#[test]
fn final_cleanup_keeps_only_stored_value_access_available() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (signal, setter) = scope.signal(1_i32).expect("fallible reactive creation");
            let stored = scope.stored(1_i32).expect("fallible reactive creation");
            let node_ref = scope.node_ref::<i32>().expect("node ref creation");
            let callback = scope
                .callback(|_: ()| Ok::<(), ()>(()))
                .expect("callback should initialize");
            let completion = scope
                .completion_once(unwind_safe(|_: ()| Ok::<(), ()>(())))
                .expect("completion registration");
            let late_cleanup_handler = handler(scope);
            let observed_in_cleanup = observed.clone();
            let scope_in_cleanup = scope;
            scope
                .on_cleanup(
                    move || {
                        assert!(!scope_in_cleanup.is_active());
                        assert_eq!(stored.update(|value| *value = 2), Ok(()));
                        assert_eq!(stored.with(|value| *value), Ok(2));
                        assert_eq!(signal.get(), Err(ReactiveError::NoSuchNode));
                        assert_eq!(setter.set(2), Err(ReactiveError::NoSuchNode));
                        assert_eq!(node_ref.get(), Err(ReactiveError::NoSuchNode));
                        assert_eq!(node_ref.set(2), Err(ReactiveError::NoSuchNode));
                        assert!(matches!(
                            callback.invoke(()),
                            Err(CallbackInvokeError::Runtime(ReactiveError::NoSuchNode))
                        ));
                        assert!(matches!(
                            scope_in_cleanup.stored(()),
                            Err(ReactiveError::NoSuchNode)
                        ));
                        assert_eq!(
                            scope_in_cleanup.on_cleanup(|| Ok(()), late_cleanup_handler),
                            Err(ReactiveError::NoSuchNode)
                        );
                        assert!(!completion.submit(()).expect("stale completion submit"));
                        observed_in_cleanup.set(true);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    assert!(observed.get());
}

#[test]
fn final_cleanup_releases_stored_value_lease_after_panic() {
    let mut runtime = Runtime::new();
    let updated = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let stored = scope.stored(1_i32).expect("fallible reactive creation");
            let updated_in_cleanup = updated.clone();
            scope
                .on_cleanup(
                    move || {
                        let panic = catch_unwind(AssertUnwindSafe(|| {
                            stored
                                .with(|_| panic!("cleanup read panic"))
                                .expect("test operation should succeed");
                        }));
                        assert!(panic.is_err());
                        stored.update(|value| *value = 2).expect("stored update");
                        updated_in_cleanup.set(true);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    assert!(updated.get());
}

#[test]
fn child_scope_is_inactive_after_scope_returns() {
    let mut runtime = Runtime::new();
    let (token, read_cell) = runtime
        .child(|scope| {
            let cell = Rc::new(Cell::new(10));
            let cell_in_callback = cell.clone();
            let token = scope
                .completion_once(unwind_safe(move |val: i32| {
                    cell_in_callback.set(val);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            (token, cell)
        })
        .expect("runtime child");

    assert_eq!(read_cell.get(), 10);
    assert!(!token.submit(20).expect("stale completion submit"));
    assert_eq!(read_cell.get(), 10);
}

#[test]
fn cleanup_order_follows_lexical_scope_order() {
    let mut runtime = Runtime::new();
    let events = Rc::new(RefCell::new(Vec::new()));

    runtime
        .child(|scope| {
            let parent_events = events.clone();
            scope
                .on_cleanup(
                    move || {
                        parent_events.borrow_mut().push("parent");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");

            scope
                .child(|child| {
                    let child_events = events.clone();
                    child
                        .on_cleanup(
                            move || {
                                child_events.borrow_mut().push("child");
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("cleanup should register");
                })
                .expect("test operation should succeed");

            assert_eq!(events.borrow().as_slice(), &["child"]);
        })
        .expect("test operation should succeed");

    assert_eq!(events.borrow().as_slice(), &["child", "parent"]);
}

#[test]
fn child_scope_panic_cleans_up_before_parent_continues() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(false));
    let parent_continued = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let cleaned_in_child = cleaned.clone();
            let panic = catch_unwind(AssertUnwindSafe(|| {
                scope
                    .child(|child| {
                        child
                            .on_cleanup(
                                move || {
                                    cleaned_in_child.set(true);
                                    Ok(())
                                },
                                handler(child),
                            )
                            .expect("cleanup should register");
                        panic!("child callback panic");
                    })
                    .expect("test operation should succeed");
            }));

            assert!(panic.is_err());
            assert!(cleaned.get());
            parent_continued.set(true);
        })
        .expect("test operation should succeed");

    assert!(parent_continued.get());
}

#[test]
fn child_callback_panic_is_not_replaced_by_cleanup_panic() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let panic = catch_unwind(AssertUnwindSafe(|| {
                scope
                    .child(|child| {
                        child
                            .on_cleanup(|| panic!("cleanup panic"), handler(child))
                            .expect("cleanup should register");
                        panic!("callback panic");
                    })
                    .expect("test operation should succeed");
            }))
            .expect_err("child callback should panic");

            assert_eq!(panic.downcast_ref::<&str>(), Some(&"callback panic"));
        })
        .expect("test operation should succeed");
}

#[test]
fn parent_effect_tracks_parent_reads_inside_child_callback() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let parent_scope = scope;
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        parent_scope
                            .child(|_| {
                                source.get().expect("test operation should succeed");
                            })
                            .expect("test operation should succeed");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_source.set(1).expect("test operation should succeed");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn parent_effect_tracks_parent_reads_inside_nested_child_callback() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("signal creation");
            let parent_scope = scope;
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        parent_scope
                            .child(|_| {
                                source.get().expect("source read");
                            })
                            .expect("child should complete");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_source.set(1).expect("signal update");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn nested_child_frames_keep_parent_tracking_at_the_outer_boundary() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (outer, set_outer) = scope.signal(0_i32).expect("outer source creation");
            let (inner, set_inner) = scope.signal(0_i32).expect("inner source creation");
            let (deep, set_deep) = scope.signal(0_i32).expect("deep source creation");
            let parent_scope = scope;
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        parent_scope
                            .child(|level1| {
                                outer.get().expect("outer read");
                                level1
                                    .child(|level2| {
                                        inner.get().expect("inner read");
                                        level2
                                            .child(|_level3| {
                                                deep.get().expect("deep read");
                                            })
                                            .expect("level3 should complete");
                                    })
                                    .expect("level2 should complete");
                            })
                            .expect("level1 should complete");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_inner.set(1).expect("inner update");
            assert_eq!(runs.get(), 2);
            set_deep.set(1).expect("deep update");
            assert_eq!(runs.get(), 3);
            set_outer.set(1).expect("outer update");
            assert_eq!(runs.get(), 4);
        })
        .expect("test operation should succeed");
}

#[test]
fn nested_child_panic_restores_the_parent_observer_frame() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let parent_scope = scope;
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        let panic = catch_unwind(AssertUnwindSafe(|| {
                            parent_scope
                                .child(|level1| {
                                    level1
                                        .child(|level2| {
                                            level2
                                                .child(|_| panic!("nested child panic"))
                                                .expect("level3 should complete");
                                        })
                                        .expect("level2 should complete");
                                })
                                .expect("level1 should complete");
                        }));
                        assert!(panic.is_err());
                        source.get().expect("source read after panic");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            assert_eq!(runs.get(), 1);
            set_source.set(1).expect("source update");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn cleanup_track_is_untracked_and_does_not_add_a_dependency() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));
    let cleanup_track_succeeded = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("source creation");
            let (other, set_other) = scope.signal(0i32).expect("other creation");
            let runs_in_effect = runs.clone();
            let cleanup_track_succeeded_in_effect = cleanup_track_succeeded.clone();
            scope
                .effect(
                    move || {
                        source.get().expect("source read");
                        let cleanup_track_succeeded_in_cleanup =
                            cleanup_track_succeeded_in_effect.clone();
                        scope
                            .on_cleanup(
                                move || {
                                    cleanup_track_succeeded_in_cleanup.set(other.get().is_ok());
                                    Ok(())
                                },
                                handler(scope),
                            )
                            .expect("cleanup registration");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            set_source.set(1).expect("source update");
            assert_eq!(runs.get(), 2);
            assert!(cleanup_track_succeeded.get());
            set_other.set(1).expect("other update");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn untrack_blocks_ordinary_reads() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let (other, set_other) = scope.signal(0_i32).expect("other creation");
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    move || {
                        scope.untrack(|| {
                            other.get().expect("untracked read");
                        });
                        source.get().expect("source read");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            set_other.set(1).expect("other update");
            assert_eq!(runs.get(), 1);
            set_source.set(1).expect("source update");
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn child_local_signal_does_not_keep_parent_effect_queued_after_exit() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let parent_scope = scope;
            let runs_in_effect = runs.clone();
            let result = catch_unwind(AssertUnwindSafe(|| {
                scope
                    .effect(
                        move || {
                            runs_in_effect.set(runs_in_effect.get() + 1);
                            parent_scope
                                .child(|child| {
                                    let (local, set_local) =
                                        child.signal(0i32).expect("fallible reactive creation");
                                    local.get().expect("test operation should succeed");
                                    set_local.set(1).expect("test operation should succeed");
                                })
                                .expect("test operation should succeed");
                            Ok(())
                        },
                        handler(scope),
                    )
                    .expect("effect should initialize");
            }));

            assert!(result.is_ok());
            assert_eq!(runs.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn cleanup_can_reenter_an_active_parent_scope() {
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

            scope
                .child(|child| {
                    child
                        .on_cleanup(
                            move || {
                                set_source.set(1).expect("test operation should succeed");
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("cleanup should register");
                })
                .expect("test operation should succeed");

            assert_eq!(seen.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn panic_in_scoped_run_still_drops_the_scope() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(false));
    let cleaned_in_scope = cleaned.clone();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime
            .child(|scope| {
                scope
                    .on_cleanup(
                        move || {
                            cleaned_in_scope.set(true);
                            Ok(())
                        },
                        handler(scope),
                    )
                    .expect("cleanup should register");
                panic!("run panic");
            })
            .expect("test operation should succeed");
    }));
    assert!(panic.is_err());
    assert!(cleaned.get());

    runtime
        .child(|scope| {
            let (signal, set_signal) = scope.signal(1i32).expect("fallible reactive creation");
            set_signal.set(2).expect("test operation should succeed");
            assert_eq!(signal.get(), Ok(2));
        })
        .expect("test operation should succeed");
}

#[test]
fn cleanup_panic_does_not_poison_runtime() {
    let mut runtime = Runtime::new();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime
            .child(|scope| {
                scope
                    .on_cleanup(|| panic!("cleanup panic"), handler(scope))
                    .expect("cleanup should register");
            })
            .expect("test operation should succeed");
    }));
    assert!(panic.is_err());

    runtime
        .child(|scope| {
            let (signal, set_signal) = scope.signal(1i32).expect("fallible reactive creation");
            set_signal.set(2).expect("test operation should succeed");
            assert_eq!(signal.get(), Ok(2));
        })
        .expect("test operation should succeed");
}

#[test]
fn cleanup_panic_does_not_skip_remaining_cleanups() {
    let mut runtime = Runtime::new();
    let remaining_cleanup_ran = Rc::new(Cell::new(false));
    let remaining_cleanup_ran_in_scope = remaining_cleanup_ran.clone();

    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime
            .child(|scope| {
                scope
                    .on_cleanup(|| panic!("first cleanup panic"), handler(scope))
                    .expect("cleanup should register");
                scope
                    .on_cleanup(
                        move || {
                            remaining_cleanup_ran_in_scope.set(true);
                            Ok(())
                        },
                        handler(scope),
                    )
                    .expect("cleanup should register");
            })
            .expect("test operation should succeed");
    }));

    assert!(panic.is_err());
    assert!(remaining_cleanup_ran.get());
}

#[test]
fn cleanup_panic_does_not_skip_other_nodes_or_root_cleanup() {
    let mut runtime = Runtime::new();
    let other_node_cleaned = Rc::new(Cell::new(false));
    let root_cleaned = Rc::new(Cell::new(false));
    let other_node_cleaned_in_scope = other_node_cleaned.clone();
    let root_cleaned_in_scope = root_cleaned.clone();

    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime
            .child(|scope| {
                let scope_copy = scope;
                scope
                    .effect(
                        move || {
                            scope_copy
                                .on_cleanup(
                                    || panic!("first node cleanup panic"),
                                    handler(scope_copy),
                                )
                                .expect("cleanup should register");
                            Ok(())
                        },
                        handler(scope),
                    )
                    .expect("effect should initialize");

                let scope_copy = scope;
                scope
                    .effect(
                        move || {
                            let cleaned = other_node_cleaned_in_scope.clone();
                            scope_copy
                                .on_cleanup(
                                    move || {
                                        cleaned.set(true);
                                        Ok(())
                                    },
                                    handler(scope_copy),
                                )
                                .expect("cleanup should register");
                            Ok(())
                        },
                        handler(scope),
                    )
                    .expect("effect should initialize");

                scope
                    .on_cleanup(
                        move || {
                            root_cleaned_in_scope.set(true);
                            Ok(())
                        },
                        handler(scope),
                    )
                    .expect("cleanup should register");
            })
            .expect("test operation should succeed");
    }));

    assert!(panic.is_err());
    assert!(other_node_cleaned.get());
    assert!(root_cleaned.get());
}

#[test]
fn scope_cleanup_can_register_another_cleanup() {
    let mut runtime = Runtime::new();
    let first_ran = Rc::new(Cell::new(false));
    let second_ran = Rc::new(Cell::new(false));
    let first_ran_in_scope = first_ran.clone();
    let second_ran_in_scope = second_ran.clone();

    runtime
        .child(|scope| {
            let scope_copy = scope;
            let second_cleanup_handler = handler(scope);
            scope
                .on_cleanup(
                    move || {
                        first_ran_in_scope.set(true);
                        let second_ran = second_ran_in_scope.clone();
                        assert_eq!(
                            scope_copy.on_cleanup(
                                move || {
                                    second_ran.set(true);
                                    Ok(())
                                },
                                second_cleanup_handler,
                            ),
                            Err(ReactiveError::NoSuchNode)
                        );
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    assert!(first_ran.get());
    assert!(!second_ran.get());
}

#[test]
fn effect_cleanup_can_register_cleanup_for_the_next_run() {
    let mut runtime = Runtime::new();
    let first_cleanup_ran = Rc::new(Cell::new(false));
    let second_cleanup_ran = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(0i32).expect("fallible reactive creation");
            let scope_copy = scope;
            let register_initial_cleanup = Rc::new(Cell::new(true));
            let first_cleanup_ran_in_effect = first_cleanup_ran.clone();
            let second_cleanup_ran_in_effect = second_cleanup_ran.clone();
            scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
                        if register_initial_cleanup.replace(false) {
                            let scope_for_cleanup = scope_copy;
                            let first_cleanup = first_cleanup_ran_in_effect.clone();
                            let second_cleanup = second_cleanup_ran_in_effect.clone();
                            scope_copy
                                .on_cleanup(
                                    move || {
                                        first_cleanup.set(true);
                                        scope_for_cleanup
                                            .on_cleanup(
                                                move || {
                                                    second_cleanup.set(true);
                                                    Ok(())
                                                },
                                                handler(scope_for_cleanup),
                                            )
                                            .expect("cleanup should register");
                                        Ok(())
                                    },
                                    handler(scope_for_cleanup),
                                )
                                .expect("cleanup should register");
                        }
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            set_source.set(1).expect("test operation should succeed");
            assert!(first_cleanup_ran.get());
            assert!(!second_cleanup_ran.get());

            set_source.set(2).expect("test operation should succeed");
            assert!(second_cleanup_ran.get());
        })
        .expect("test operation should succeed");
}

#[test]
fn child_cleanup_panic_still_flushes_parent_queue() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let (source, set_source) = scope
                .signal(RefCell::new(0i32))
                .expect("fallible reactive creation");
            let runs_in_effect = runs.clone();
            scope
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
                    handler(scope),
                )
                .expect("effect should initialize");
            assert_eq!(runs.get(), 1);

            let panic = catch_unwind(AssertUnwindSafe(|| {
                scope
                    .child(|child| {
                        let source_in_cleanup = source;
                        let setter_in_cleanup = set_source;
                        let runs_in_cleanup = runs.clone();
                        child
                            .on_cleanup(
                                move || {
                                    source_in_cleanup
                                        .with(|_| {
                                            notify(&setter_in_cleanup)
                                                .expect("test operation should succeed");
                                            assert_eq!(runs_in_cleanup.get(), 1);
                                            panic!("child cleanup panic");
                                        })
                                        .expect("test operation should succeed");
                                    Ok(())
                                },
                                handler(child),
                            )
                            .expect("cleanup should register");
                    })
                    .expect("test operation should succeed");
            }));

            assert!(panic.is_err());
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn completion_token_accepts_active_submissions_and_rejects_after_scope() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let seen_in_scope = seen.clone();
    let token = runtime
        .child(|scope| {
            let seen_in_callback = seen_in_scope.clone();
            let token = scope
                .completion_sender(unwind_safe(move |value: i32| {
                    seen_in_callback.set(value);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            assert!(token.submit(1).expect("completion submit"));
            token
        })
        .expect("runtime child");

    assert_eq!(seen.get(), 1);
    assert!(!token.submit(2).expect("stale completion submit"));
    assert_eq!(seen.get(), 1);
}

#[test]
fn completion_token_rejects_submission_after_scope_deactivation() {
    let mut runtime = Runtime::new();
    let callback_called = Rc::new(Cell::new(false));

    runtime
        .child(|scope| {
            scope
                .child(|child| {
                    let callback_called_in_child = callback_called.clone();
                    let token = child
                        .completion_once(unwind_safe(move |_: i32| {
                            callback_called_in_child.set(true);
                            Ok::<(), ()>(())
                        }))
                        .expect("completion registration");
                    let child_scope = child;
                    child
                        .effect(
                            move || {
                                let token_in_cleanup = token.clone();
                                child_scope
                                    .on_cleanup(
                                        move || {
                                            assert!(
                                                !token_in_cleanup
                                                    .submit(1)
                                                    .expect("stale completion submit")
                                            );
                                            Ok(())
                                        },
                                        handler(child_scope),
                                    )
                                    .expect("cleanup should register");
                                Ok(())
                            },
                            handler(child),
                        )
                        .expect("effect should initialize");
                })
                .expect("test operation should succeed");
        })
        .expect("test operation should succeed");

    assert!(!callback_called.get());
}

#[test]
fn lexical_completion_can_capture_scope_local_data() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let local = String::from("scoped");
            let seen_in_callback = seen.clone();
            let token = scope
                .completion_once(unwind_safe(move |value: i32| {
                    assert_eq!(local, "scoped");
                    seen_in_callback.set(value);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            assert!(token.submit(7).expect("completion submit"));
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 7);
}

#[test]
fn handles_are_invalid_after_their_scope_and_runtimes_are_isolated() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    first
        .child(|scope| {
            let (signal, _) = scope.signal(1i32).expect("fallible reactive creation");
            assert_eq!(signal.get(), Ok(1));
            second
                .child(|other| {
                    let (other_signal, _) = other.signal(2i32).expect("fallible reactive creation");
                    assert_eq!(other_signal.get(), Ok(2));
                    assert_eq!(signal.get(), Ok(1));
                })
                .expect("test operation should succeed");
            assert_eq!(signal.get(), Ok(1));
        })
        .expect("test operation should succeed");

    let mut gone = Runtime::new();
    let token = gone
        .child(|scope| {
            scope
                .child(|child| {
                    let (signal, _) = child.signal(1i32).expect("fallible reactive creation");
                    assert_eq!(signal.get(), Ok(1));
                })
                .expect("test operation should succeed");
            scope
                .completion_once(unwind_safe(|_: i32| Ok::<(), ()>(())))
                .expect("completion registration")
        })
        .expect("runtime child");
    assert!(!token.submit(1).expect("stale completion submit"));

    assert_eq!(
        ReactiveError::NoSuchNode.to_string(),
        "节点不存在或所属 scope 已结束"
    );
}
