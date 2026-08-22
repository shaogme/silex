#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{EffectPhase, ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope, E: 'scope>(owner: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, E> {
    owner
        .error_handler(|_| {})
        .expect("error handler registration")
}

#[test]
fn normal_effects_run_before_post_flush_effects() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let log = Rc::new(RefCell::new(Vec::new()));
            let normal_log = log.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get()?;
                        normal_log.borrow_mut().push("normal");
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("normal effect creation");
            let post_log = log.clone();
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        source.get()?;
                        post_log.borrow_mut().push("post");
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("post effect creation");

            log.borrow_mut().clear();
            set_source.set(1).expect("source update");
            assert_eq!(*log.borrow(), ["normal", "post"]);
        })
        .expect("transient scope");
}

#[test]
fn post_flush_effects_preserve_ordered_edge_registration() {
    fn assert_order(reverse: bool) {
        let mut runtime = Runtime::new();
        runtime
            .with_transient(|scope| {
                let (source, set_source) = scope.signal(0_i32).expect("source creation");
                let log = Rc::new(RefCell::new(Vec::new()));
                let first_log = log.clone();
                let second_log = log.clone();
                let first = move || {
                    source.get()?;
                    first_log.borrow_mut().push("first");
                    Ok(())
                };
                let second = move || {
                    source.get()?;
                    second_log.borrow_mut().push("second");
                    Ok(())
                };
                if reverse {
                    scope
                        .effect(
                            EffectPhase::PostFlush,
                            second,
                            handler::<ReactiveError>(scope),
                        )
                        .expect("second post effect creation");
                    scope
                        .effect(
                            EffectPhase::PostFlush,
                            first,
                            handler::<ReactiveError>(scope),
                        )
                        .expect("first post effect creation");
                } else {
                    scope
                        .effect(
                            EffectPhase::PostFlush,
                            first,
                            handler::<ReactiveError>(scope),
                        )
                        .expect("first post effect creation");
                    scope
                        .effect(
                            EffectPhase::PostFlush,
                            second,
                            handler::<ReactiveError>(scope),
                        )
                        .expect("second post effect creation");
                }

                log.borrow_mut().clear();
                set_source.set(1).expect("source update");
                let expected = if reverse {
                    ["second", "first"]
                } else {
                    ["first", "second"]
                };
                assert_eq!(*log.borrow(), expected);
            })
            .expect("transient scope");
    }

    assert_order(false);
    assert_order(true);
}

#[test]
fn post_flush_write_reenters_normal_queue_before_remaining_post_tasks() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let (marker, set_marker) = scope.signal(0_i32).expect("marker creation");
            let log = Rc::new(RefCell::new(Vec::new()));

            let normal_log = log.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        marker.get()?;
                        normal_log.borrow_mut().push("normal");
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("normal effect creation");

            let first_log = log.clone();
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        if source.get()? == 1 {
                            first_log.borrow_mut().push("post-first");
                            set_marker.set(1)?;
                        }
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("first post effect creation");

            let second_log = log.clone();
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        source.get()?;
                        second_log.borrow_mut().push("post-second");
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("second post effect creation");

            log.borrow_mut().clear();
            set_source.set(1).expect("source update");
            assert_eq!(*log.borrow(), ["post-first", "normal", "post-second"]);
        })
        .expect("transient scope");
}

#[test]
fn computed_chain_is_evaluated_before_a_post_flush_observer() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(1_i32).expect("source creation");
            let log = Rc::new(RefCell::new(Vec::new()));
            let computed_log = log.clone();
            let doubled = scope
                .computed(
                    move || {
                        computed_log.borrow_mut().push("computed");
                        Ok(source.get()? * 2)
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("computed creation");
            let observer_log = log.clone();
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        observer_log.borrow_mut().push(
                            if doubled.get().expect("computed read") == 4 {
                                "observer"
                            } else {
                                "wrong"
                            },
                        );
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("post observer creation");

            log.borrow_mut().clear();
            set_source.set(2).expect("source update");
            assert_eq!(*log.borrow(), ["computed", "observer"]);
        })
        .expect("transient scope");
}

#[test]
fn closing_an_owner_removes_pending_post_flush_tasks() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("root owner");
    let child = root.access().create_child().expect("child owner creation");
    let child_access = child.access();
    let (source, set_source) = child_access.signal(0_i32).expect("source creation");
    let runs = Rc::new(Cell::new(0));
    let runs_in_effect = runs.clone();
    child_access
        .effect(
            EffectPhase::PostFlush,
            move || {
                source.get()?;
                runs_in_effect.set(runs_in_effect.get() + 1);
                Ok(())
            },
            handler::<ReactiveError>(child_access),
        )
        .expect("post effect creation");

    root.access()
        .batch(|| {
            set_source.set(1).expect("source update");
            child.close().expect("child close");
        })
        .expect("batch flush");
    assert_eq!(runs.get(), 1);
    root.close().expect("root close");
}

#[test]
fn post_flush_callback_errors_are_dispatched() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let errors = Rc::new(RefCell::new(Vec::new()));
            let errors_in_handler = errors.clone();
            let error_handler = scope
                .error_handler(move |error: &'static str| {
                    errors_in_handler.borrow_mut().push(error);
                })
                .expect("error handler registration");
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        if source.get().map_err(|_| "post failure")? == 1 {
                            Err("post failure")
                        } else {
                            Ok(())
                        }
                    },
                    error_handler,
                )
                .expect("post effect creation");

            set_source.set(1).expect("source update");
            assert_eq!(*errors.borrow(), ["post failure"]);
        })
        .expect("transient scope");
}

#[test]
fn post_flush_panic_recovers_the_queue() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let should_panic = Rc::new(Cell::new(false));
            let runs = Rc::new(Cell::new(0));
            let should_panic_in_effect = should_panic.clone();
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        let value = source.get()?;
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        if value == 1 && should_panic_in_effect.get() {
                            panic!("post flush panic");
                        }
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("post effect creation");

            should_panic.set(true);
            let panic = catch_unwind(AssertUnwindSafe(|| {
                set_source.set(1).expect("source update");
            }));
            assert!(panic.is_err());
            should_panic.set(false);
            set_source.set(2).expect("retry source update");
            assert_eq!(runs.get(), 3);
        })
        .expect("transient scope");
}

#[test]
fn post_flush_nonconvergence_reports_its_phase() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            let (relay, set_relay) = scope.signal(0_i32).expect("relay creation");
            let enabled = Rc::new(Cell::new(false));
            let ticker = Rc::new(Cell::new(0_i32));
            let enabled_in_source = enabled.clone();
            let ticker_in_source = ticker.clone();
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        source.get()?;
                        if enabled_in_source.get() {
                            let next = ticker_in_source.get().saturating_add(1);
                            ticker_in_source.set(next);
                            set_relay.set(next)?;
                        }
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("source post effect creation");

            let enabled_in_relay = enabled.clone();
            let ticker_in_relay = ticker.clone();
            scope
                .effect(
                    EffectPhase::PostFlush,
                    move || {
                        relay.get()?;
                        if enabled_in_relay.get() {
                            let next = ticker_in_relay.get().saturating_add(1);
                            ticker_in_relay.set(next);
                            set_source.set(next)?;
                        }
                        Ok(())
                    },
                    handler::<ReactiveError>(scope),
                )
                .expect("relay post effect creation");

            enabled.set(true);
            let result = scope
                .batch(|| set_source.set(1))
                .expect_err("nonconvergent queue");
            assert!(matches!(
                result,
                ReactiveError::NonConvergent {
                    last_phase: Some(EffectPhase::PostFlush),
                    ..
                }
            ));
        })
        .expect("transient scope");
}

#[cfg(feature = "test-support")]
#[test]
fn mixed_phase_snapshot_records_queue_high_water() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("source creation");
            for phase in [
                EffectPhase::Normal,
                EffectPhase::Normal,
                EffectPhase::PostFlush,
                EffectPhase::PostFlush,
            ] {
                scope
                    .effect(
                        phase,
                        move || {
                            source.get()?;
                            Ok(())
                        },
                        handler::<ReactiveError>(scope),
                    )
                    .expect("phase effect creation");
            }

            set_source.set(1).expect("source update");
            let snapshot = scope.runtime_snapshot().expect("runtime snapshot");
            assert!(snapshot.queue_high_water >= 4);
            assert_eq!(snapshot.queue, 0);
            assert!(snapshot.queue_recovery);
        })
        .expect("transient scope");
}
