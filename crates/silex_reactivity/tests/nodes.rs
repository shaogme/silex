#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{
    Callback, CallbackInvokeError, ComputationInitError, Computed, EffectHandle, EffectPhase,
    ErrorHandlerRef, ErrorHandlerToken, NodeRef, OwnerAccess, ReactiveError, ReadSignal, Runtime,
    Signal, StoredValue, WriteSignal,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

struct ReenterOnDrop<'scope> {
    setter: WriteSignal<'scope, i32>,
    called: Rc<Cell<bool>>,
    error: Rc<Cell<Option<ReactiveError>>>,
}

struct ReadOnDrop<'scope> {
    probe: ReadSignal<'scope, i32>,
    drops: Rc<Cell<usize>>,
}

impl PartialEq for ReadOnDrop<'_> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Drop for ReadOnDrop<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(matches!(
            self.probe.get(),
            Ok(_) | Err(ReactiveError::NoSuchNode)
        ));
    }
}

struct DropEvent {
    label: &'static str,
    events: Rc<RefCell<Vec<&'static str>>>,
}

struct ErrorDropCounter(Rc<Cell<usize>>);

impl Drop for ErrorDropCounter {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[derive(Debug)]
enum CallbackError {
    Rejected,
}

impl Drop for DropEvent {
    fn drop(&mut self) {
        self.events.borrow_mut().push(self.label);
    }
}

impl Drop for ReenterOnDrop<'_> {
    fn drop(&mut self) {
        self.called.set(true);
        self.error.set(self.setter.set(1).err());
    }
}

#[test]
fn all_public_node_capabilities_are_copy() {
    fn assert_copy<T: Copy>(_: T) {}

    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let signal = scope.signal(0i32).expect("fallible reactive creation");
            let read = signal.read();
            let write = signal.write();
            let memo = scope
                .computed(
                    move || Ok(read.get().expect("reactive read")),
                    handler(scope),
                )
                .expect("memo creation");
            let derived = scope
                .computed_always(move || Ok(1i32), handler(scope))
                .expect("derived creation");
            let effect = scope
                .effect(EffectPhase::Normal, || Ok(()), handler(scope))
                .expect("effect should initialize");
            let stored = scope.stored(1i32).expect("fallible reactive creation");
            let callback = scope
                .callback(|_: ()| Ok::<(), ReactiveError>(()))
                .expect("callback should initialize");
            let node_ref = scope.node_ref::<i32>().expect("fallible reactive creation");
            let handler_ref = handler(scope).view();

            assert_copy(scope);
            assert_copy(handler_ref);
            let _: ErrorHandlerRef<'_, ()> = handler_ref;
            assert_copy(read);
            assert_copy(write);
            assert_copy(signal);
            assert_copy(memo);
            assert_copy(derived);
            assert_copy(effect);
            assert_copy(stored);
            assert_copy(callback);
            assert_copy(node_ref);

            let _: Option<ReadSignal<'_, i32>> = Some(read);
            let _: Option<WriteSignal<'_, i32>> = Some(write);
            let _: Option<Computed<'_, i32, ()>> = Some(memo);
            let _: Option<Computed<'_, i32, ()>> = Some(derived);
            let _: Option<EffectHandle<'_>> = Some(effect);
            let _: Option<StoredValue<'_, i32>> = Some(stored);
            let _: Option<Callback<'_, ()>> = Some(callback);
            let _: Option<NodeRef<'_, i32>> = Some(node_ref);
        })
        .expect("test operation should succeed");
}

#[test]
fn signal_pair_round_trip_preserves_node_identity() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let signal = scope.signal(1_i32).expect("signal creation");
            let rebuilt = Signal::from_pair((signal.read(), signal.write()))
                .expect("signal pair should be valid");

            assert_eq!(rebuilt.get(), Ok(1));
            rebuilt.set(2).expect("signal write");
            assert_eq!(rebuilt.get(), Ok(2));
        })
        .expect("scope execution");
}

#[test]
fn signal_pair_rejects_capabilities_from_different_nodes() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let first = scope.signal(1_i32).expect("first signal");
            let second = scope.signal(2_i32).expect("second signal");

            assert!(matches!(
                Signal::from_pair((first.read(), second.write())),
                Err(ReactiveError::InvariantViolation)
            ));
        })
        .expect("scope execution");
}

#[test]
fn stored_callback_and_node_ref_are_scope_owned() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let stored = scope
                .stored(String::from("before"))
                .expect("fallible reactive creation");
            stored
                .update(|value| value.push_str(" after"))
                .expect("test operation should succeed");
            assert!(
                stored
                    .with(|value| value == "before after")
                    .expect("stored read")
            );

            let called = Rc::new(Cell::new(0));
            let called_in_callback = called.clone();
            let callback = scope
                .callback(move |_: ()| {
                    called_in_callback.set(called_in_callback.get() + 1);
                    Ok::<(), ReactiveError>(())
                })
                .expect("callback should initialize");
            callback.invoke(()).expect("callback should be alive");
            assert_eq!(called.get(), 1);

            let reference = scope.node_ref::<u32>().expect("fallible reactive creation");
            assert_eq!(reference.get(), Ok(None));
            reference.set(7).expect("node ref should be writable");
            assert_eq!(reference.get(), Ok(Some(7)));
            reference.clear().expect("node ref should be clearable");
            assert_eq!(reference.get(), Ok(None));
        })
        .expect("test operation should succeed");
}

#[test]
fn copy_capabilities_return_no_such_node_after_child_release() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let child = root.create_child().expect("child creation");
    let access = child.access();
    let handler = access
        .error_handler(|_: ()| {})
        .expect("handler registration");
    let read = access.signal(1_i32).expect("signal creation");
    let rw = access.signal(2_i32).expect("signal creation");
    let computed = access
        .computed(|| Ok::<i32, ()>(3), handler.view())
        .expect("computed creation");
    let node_ref = access.node_ref::<i32>().expect("node ref creation");
    let stored = access.stored(4_i32).expect("stored creation");
    let callback = access
        .callback(|_: ()| Ok::<(), ReactiveError>(()))
        .expect("callback creation");

    child.close().expect("child close");

    assert_eq!(read.get(), Err(ReactiveError::NoSuchNode));
    assert_eq!(read.set(5), Err(ReactiveError::NoSuchNode));
    assert_eq!(rw.read().get(), Err(ReactiveError::NoSuchNode));
    assert_eq!(rw.write().set(5), Err(ReactiveError::NoSuchNode));
    assert!(matches!(
        computed.get(),
        Err(CallbackInvokeError::Runtime(ReactiveError::NoSuchNode))
    ));
    assert_eq!(node_ref.get(), Err(ReactiveError::NoSuchNode));
    assert_eq!(stored.with(|value| *value), Err(ReactiveError::NoSuchNode));
    assert!(matches!(
        callback.invoke(()),
        Err(CallbackInvokeError::Runtime(ReactiveError::NoSuchNode))
    ));
    root.close().expect("root close");
}

#[test]
fn initial_error_slot_releases_the_user_error_once() {
    let mut runtime = Runtime::new();
    let drops = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let handler = scope
                .error_handler(|_: ErrorDropCounter| {})
                .expect("handler");
            let result = scope.computed(
                {
                    let drops = drops.clone();
                    move || Err::<i32, ErrorDropCounter>(ErrorDropCounter(drops.clone()))
                },
                handler.view(),
            );
            assert!(matches!(&result, Err(ComputationInitError::Initial(_))));
            assert_eq!(drops.get(), 0);
            drop(result);
            assert_eq!(drops.get(), 1);
        })
        .expect("transient scope should close");
}

#[test]
fn callback_user_error_is_returned_and_callback_remains_reusable() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(Cell::new(0));
    let seen = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let calls_in_callback = calls.clone();
            let seen_in_callback = seen.clone();
            let callback = scope
                .callback(move |value: i32| {
                    calls_in_callback.set(calls_in_callback.get() + 1);
                    if value == 0 {
                        Err(CallbackError::Rejected)
                    } else {
                        seen_in_callback.set(value);
                        Ok(())
                    }
                })
                .expect("callback should initialize");

            assert!(matches!(
                callback.invoke(0),
                Err(CallbackInvokeError::User(CallbackError::Rejected))
            ));
            assert_eq!(calls.get(), 1);
            assert_eq!(seen.get(), 0);

            callback.invoke(7).expect("callback should remain reusable");
            assert_eq!(calls.get(), 2);
            assert_eq!(seen.get(), 7);
        })
        .expect("test operation should succeed");
}

#[test]
fn callback_runtime_error_and_user_reactive_error_are_distinct() {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|scope| {
            let callback = scope
                .callback(|_: ()| Err::<(), ReactiveError>(ReactiveError::NoSuchNode))
                .expect("callback should initialize");

            assert!(matches!(
                callback.invoke(()),
                Err(CallbackInvokeError::User(ReactiveError::NoSuchNode))
            ));
        })
        .expect("test operation should succeed");
}

#[test]
fn callback_dispatches_user_error_once_after_releasing_its_lease() {
    let mut runtime = Runtime::new();
    let handler_calls = Rc::new(Cell::new(0));
    let observed = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let signal = scope.signal(0_i32).expect("fallible reactive creation");
            let handler_calls_in_handler = handler_calls.clone();
            let handler = scope
                .error_handler(move |_: &'static str| {
                    handler_calls_in_handler.set(handler_calls_in_handler.get() + 1);
                    signal.set(1).expect("signal update");
                })
                .expect("handler registration");
            let callback = scope
                .callback(|_: ()| Err::<(), &'static str>("callback failed"))
                .expect("callback should initialize");

            assert!(matches!(
                callback.invoke(()),
                Err(CallbackInvokeError::User("callback failed"))
            ));
            assert_eq!(handler_calls.get(), 0);

            callback
                .dispatch((), handler)
                .expect("error handler should consume the callback error");
            observed.set(signal.get().expect("reactive read"));
        })
        .expect("test operation should succeed");

    assert_eq!(handler_calls.get(), 1);
    assert_eq!(observed.get(), 1);
}

#[test]
fn recursive_callback_invocation_reports_borrow_conflict() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let slot: Rc<RefCell<Option<Callback<'_, (), ReactiveError>>>> =
                Rc::new(RefCell::new(None));
            let slot_in_callback = slot.clone();
            let callback = scope
                .callback(move |_: ()| {
                    let nested = slot_in_callback
                        .borrow()
                        .as_ref()
                        .copied()
                        .expect("callback should be initialized");
                    assert!(matches!(
                        nested.invoke(()),
                        Err(CallbackInvokeError::Runtime(ReactiveError::BorrowConflict))
                    ));
                    Ok::<(), ReactiveError>(())
                })
                .expect("callback should initialize");
            *slot.borrow_mut() = Some(callback);
            callback.invoke(()).expect("outer callback should succeed");
        })
        .expect("test operation should succeed");
}

#[test]
fn callback_panic_keeps_callback_available_for_the_next_invoke() {
    let mut runtime = Runtime::new();
    let called = Rc::new(Cell::new(0));
    let should_panic = Rc::new(Cell::new(true));

    runtime
        .with_transient(|scope| {
            let called_in_callback = called.clone();
            let panic_in_callback = should_panic.clone();
            let callback = scope
                .callback(move |_: ()| {
                    if panic_in_callback.replace(false) {
                        panic!("callback panic");
                    }
                    called_in_callback.set(called_in_callback.get() + 1);
                    Ok::<(), ReactiveError>(())
                })
                .expect("callback should initialize");

            let panic = catch_unwind(AssertUnwindSafe(|| {
                callback.invoke(()).expect("callback exists");
            }));
            assert!(panic.is_err());
            callback.invoke(()).expect("callback should be restored");
        })
        .expect("test operation should succeed");

    assert_eq!(called.get(), 1);
}

#[test]
fn stored_update_panic_keeps_the_stored_value_and_releases_the_lease() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let stored = scope
                .stored(String::from("before"))
                .expect("fallible reactive creation");
            let panic = catch_unwind(AssertUnwindSafe(|| {
                stored
                    .update(|_| panic!("stored update panic"))
                    .expect("test operation should succeed");
            }));
            assert!(panic.is_err());
            assert!(stored.with(|value| value == "before").expect("stored read"));

            stored
                .update(|value| value.push_str(" after"))
                .expect("test operation should succeed");
            assert!(
                stored
                    .with(|value| value == "before after")
                    .expect("stored read")
            );
        })
        .expect("test operation should succeed");
}

#[test]
fn updating_one_signal_can_read_another_signal() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let source = scope.signal(1i32).expect("fallible reactive creation");
            let other = scope.signal(2i32).expect("fallible reactive creation");
            source
                .update(|value| {
                    *value += other.get().expect("reactive read");
                })
                .expect("updating one signal should release state borrow");
            assert_eq!(source.get(), Ok(3));

            other.set(4).expect("test operation should succeed");
            source
                .update(|value| *value += other.get().expect("reactive read"))
                .expect("signal update");
            assert_eq!(source.get(), Ok(7));
        })
        .expect("test operation should succeed");
}

#[test]
fn updating_another_signal_during_read_defers_effect_flush() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0i32).expect("fallible reactive creation");
            let other = scope.signal(0i32).expect("fallible reactive creation");
            let runs_in_effect = runs.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        source.get().expect("test operation should succeed");
                        other.get().expect("test operation should succeed");
                        runs_in_effect.set(runs_in_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            let result = catch_unwind(AssertUnwindSafe(|| {
                source
                    .read()
                    .with(|_| other.set(1).expect("signal update"))
                    .expect("reactive read");
            }));

            assert!(result.is_ok());
            assert_eq!(runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn computation_payload_drop_observes_disposed_scope() {
    let mut runtime = Runtime::new();
    let called = Rc::new(Cell::new(false));
    let error = Rc::new(Cell::new(None));

    runtime
        .with_transient(|scope| {
            let scope_copy = scope;
            let called_in_outer = called.clone();
            let error_in_outer = error.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        let source = scope_copy.signal(0i32).expect("fallible reactive creation");
                        let guard = ReenterOnDrop {
                            setter: source.write(),
                            called: called_in_outer.clone(),
                            error: error_in_outer.clone(),
                        };
                        scope_copy
                            .effect(
                                EffectPhase::Normal,
                                move || {
                                    std::hint::black_box(&guard);
                                    Ok(())
                                },
                                handler(scope_copy),
                            )
                            .expect("nested effect should initialize");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
        })
        .expect("test operation should succeed");
    assert!(called.get());
    assert_eq!(error.get(), Some(ReactiveError::NoSuchNode));
}

#[test]
fn nested_memo_child_payload_drop_does_not_track_the_outer_observer() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let outer_source = scope.signal(0i32).expect("fallible reactive creation");
            let inner_source = scope.signal(0i32).expect("fallible reactive creation");
            let probe = scope.signal(0i32).expect("fallible reactive creation");
            let drops = Rc::new(Cell::new(0));
            let first_inner_run = Rc::new(Cell::new(true));
            let scope_for_child = scope;
            let probe_for_child = probe;
            let drops_in_child = drops.clone();
            let inner = scope
                .computed(
                    move || {
                        let value = inner_source.get().expect("reactive read");
                        if first_inner_run.replace(false) {
                            scope_for_child
                                .signal(ReadOnDrop {
                                    probe: probe_for_child.read(),
                                    drops: drops_in_child.clone(),
                                })
                                .expect("test operation should succeed");
                        }
                        Ok(value)
                    },
                    handler(scope),
                )
                .expect("memo creation");

            let outer_runs = Rc::new(Cell::new(0));
            let refresh_inner = Rc::new(Cell::new(false));
            let outer_inner = inner;
            let outer_source_in_effect = outer_source;
            let outer_runs_in_effect = outer_runs.clone();
            let refresh_inner_in_effect = refresh_inner.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        outer_source_in_effect
                            .get()
                            .expect("test operation should succeed");
                        outer_runs_in_effect.set(outer_runs_in_effect.get() + 1);
                        if refresh_inner_in_effect.replace(false) {
                            inner_source.set(1).expect("test operation should succeed");
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
            assert_eq!(drops.get(), 0);

            refresh_inner.set(true);
            outer_source.set(1).expect("test operation should succeed");

            assert_eq!(outer_runs.get(), 2);
            assert_eq!(drops.get(), 1);

            probe.set(1).expect("test operation should succeed");
            assert_eq!(outer_runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn nested_memo_result_drop_does_not_track_the_outer_observer() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let outer_source = scope.signal(0i32).expect("fallible reactive creation");
            let inner_source = scope.signal(0i32).expect("fallible reactive creation");
            let probe = scope.signal(0i32).expect("fallible reactive creation");
            let drops = Rc::new(Cell::new(0));
            let inner = scope
                .computed(
                    {
                        let drops = drops.clone();
                        move || {
                            inner_source.get().expect("reactive read");
                            Ok(ReadOnDrop {
                                probe: probe.read(),
                                drops: drops.clone(),
                            })
                        }
                    },
                    handler(scope),
                )
                .expect("memo creation");

            let outer_runs = Rc::new(Cell::new(0));
            let refresh_inner = Rc::new(Cell::new(false));
            let outer_inner = inner;
            let outer_source_in_effect = outer_source;
            let outer_runs_in_effect = outer_runs.clone();
            let refresh_inner_in_effect = refresh_inner.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        outer_source_in_effect
                            .get()
                            .expect("test operation should succeed");
                        outer_runs_in_effect.set(outer_runs_in_effect.get() + 1);
                        if refresh_inner_in_effect.replace(false) {
                            inner_source.set(1).expect("test operation should succeed");
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
            assert_eq!(drops.get(), 0);

            refresh_inner.set(true);
            outer_source.set(1).expect("test operation should succeed");

            assert_eq!(outer_runs.get(), 2);
            assert_eq!(drops.get(), 1);

            probe.set(1).expect("test operation should succeed");
            assert_eq!(outer_runs.get(), 2);
        })
        .expect("test operation should succeed");
}

#[test]
fn child_payloads_drop_before_parent_computation_payload() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|scope| {
            let scope_copy = scope;
            let parent_event = DropEvent {
                label: "parent",
                events: events.clone(),
            };
            let child_events = events.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        std::hint::black_box(&parent_event);
                        let signal_event = DropEvent {
                            label: "signal",
                            events: child_events.clone(),
                        };
                        scope_copy
                            .signal(signal_event)
                            .expect("fallible reactive creation");

                        let stored_event = DropEvent {
                            label: "stored",
                            events: child_events.clone(),
                        };
                        scope_copy
                            .stored(stored_event)
                            .expect("fallible reactive creation");

                        let callback_event = DropEvent {
                            label: "callback",
                            events: child_events.clone(),
                        };
                        scope_copy
                            .callback(move |_: ()| {
                                std::hint::black_box(&callback_event);
                                Ok::<(), ReactiveError>(())
                            })
                            .expect("callback should initialize");

                        let node_ref = scope_copy
                            .node_ref::<DropEvent>()
                            .expect("node ref creation");
                        node_ref
                            .set(DropEvent {
                                label: "node_ref",
                                events: child_events.clone(),
                            })
                            .expect("node ref type should match");
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
        })
        .expect("test operation should succeed");
    let events = events.borrow();
    assert_eq!(events.len(), 5);
    let parent_position = events
        .iter()
        .position(|label| *label == "parent")
        .expect("parent payload should drop");
    for label in ["signal", "stored", "callback", "node_ref"] {
        let position = events
            .iter()
            .position(|event| *event == label)
            .expect("child payload should drop");
        assert!(position < parent_position, "{label} dropped after parent");
    }
}

#[test]
fn child_callback_payload_drop_can_schedule_an_active_parent_effect() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let called = Rc::new(Cell::new(false));
    let error = Rc::new(Cell::new(None));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0i32).expect("fallible reactive creation");
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        seen_in_effect.set(source.get().expect("reactive read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            let setter = source;
            scope
                .with_transient(|child| {
                    let drop_probe = ReenterOnDrop {
                        setter: setter.write(),
                        called: called.clone(),
                        error: error.clone(),
                    };
                    let _callback = child
                        .callback(move |_: ()| {
                            std::hint::black_box(&drop_probe);
                            Ok::<(), ReactiveError>(())
                        })
                        .expect("callback should initialize");
                })
                .expect("test operation should succeed");

            assert_eq!(seen.get(), 1);
        })
        .expect("test operation should succeed");

    assert!(called.get());
    assert_eq!(error.get(), None);
}

#[test]
fn stored_value_update_flushes_after_the_write_lease_is_released() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let source = scope.signal(0i32).expect("fallible reactive creation");
            let stored = scope.stored(0i32).expect("fallible reactive creation");
            let seen_in_effect = seen.clone();
            scope
                .effect(
                    EffectPhase::Normal,
                    move || {
                        seen_in_effect.set(source.get().expect("reactive read"));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");

            stored
                .update(|value| {
                    *value = 1;
                    source.set(1).expect("test operation should succeed");
                })
                .expect("test operation should succeed");

            assert_eq!(seen.get(), 1);
        })
        .expect("test operation should succeed");
}
