use silex_core::{
    Callback, ErrorHandler, Memo, NodeRef, ReactiveError, ReadSignal, Runtime, RwSignal, Rx,
    RxDefault, RxFrom, RxRead, Scope, Signal, SilexError, SilexErrorKind, SilexResult, StoredValue,
    rx,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn scoped_primitives_propagate_without_raw_handles() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let (count, set_count) = scope.signal(1i32).expect("signal should initialize");
        let promoted = scope
            .promote(count, handler(scope))
            .expect("promotion should initialize");
        let doubled = promoted
            .map(|value| value * 2, handler(scope))
            .expect("derived map should initialize");
        let memo = scope
            .memo_from(doubled.runtime_inputs(), move |_| {
                doubled.get().expect("doubled value should be readable") + 1
            })
            .expect("memo should initialize");

        assert_eq!(doubled.get().expect("doubled value should be readable"), 2);
        assert_eq!(memo.get().expect("memo should be readable"), 3);
        set_count.set(4).expect("signal should be writable");
        assert_eq!(doubled.get().expect("doubled value should be readable"), 8);
        assert_eq!(memo.get().expect("memo should be readable"), 9);

        let stored = scope
            .stored(String::from("stored"))
            .expect("stored value should initialize");
        assert!(
            stored
                .with(|value| value == "stored")
                .expect("stored should be readable")
        );
        stored
            .update(|value| value.push_str(" value"))
            .expect("stored should be writable");
        assert!(
            stored
                .with(|value| value == "stored value")
                .expect("stored should be readable")
        );
    });
}

#[test]
fn scope_memo_and_derived_have_no_input_convenience_entries() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let (source, set_source) = scope.signal(2i32).expect("signal should initialize");
        let derived = scope
            .derived(move || source.get().map(|value| value * 2), handler(scope))
            .expect("derived should initialize");
        let memo = scope
            .memo(move |_| derived.get().expect("derived value should be readable") + 1)
            .expect("memo should initialize");

        assert_eq!(derived.get().expect("derived value should be readable"), 4);
        assert_eq!(memo.get().expect("memo should be readable"), 5);
        set_source.set(6).expect("signal should be writable");
        assert_eq!(derived.get().expect("derived value should be readable"), 12);
        assert_eq!(memo.get().expect("memo should be readable"), 13);
    });
}

#[test]
fn owned_scope_exposes_an_owner_bound_effect_only() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32).expect("signal should initialize");
        let runs = Rc::new(Cell::new(0));
        let runs_for_effect = runs.clone();
        let owner = scope.owned_scope().expect("owner should initialize");
        let _effect = owner
            .effect(
                move || {
                    let _ = source.get();
                    runs_for_effect.set(runs_for_effect.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("owned effect should register");

        set_source.set(2).expect("signal should be writable");
        assert_eq!(runs.get(), 2);
        owner.dispose().expect("owned scope should dispose");
        set_source.set(3).expect("signal should be writable");
        assert_eq!(runs.get(), 2);
    });
}

#[test]
fn lexical_effect_is_direct_and_tracks_dependencies() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let (value, set_value) = scope.signal(1i32).expect("signal should initialize");
        let seen = Rc::new(Cell::new(0));
        let seen_for_effect = seen.clone();

        let _effect = scope
            .effect(
                move || {
                    seen_for_effect.set(value.get()?);
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should register");

        assert_eq!(seen.get(), 1);
        set_value.set(4).expect("signal should be writable");
        assert_eq!(seen.get(), 4);
    });
}

#[test]
fn previous_effect_commits_the_last_returned_value() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32).expect("signal should initialize");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_for_effect = seen.clone();

        let _effect = scope
            .effect_with_previous(
                move |previous: Option<&i32>| {
                    seen_for_effect.borrow_mut().push(previous.copied());
                    source.get()
                },
                handler(scope),
            )
            .expect("previous effect should register");

        set_source.set(2).expect("signal should be writable");
        set_source.set(3).expect("signal should be writable");
        assert_eq!(*seen.borrow(), vec![None, Some(1), Some(2)]);
    });
}

#[test]
fn previous_effect_resets_after_a_panicking_run() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32).expect("signal should initialize");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_for_effect = seen.clone();
        let should_panic = Rc::new(Cell::new(false));
        let should_panic_for_effect = should_panic.clone();

        let _effect = scope
            .effect_with_previous(
                move |previous: Option<&i32>| {
                    seen_for_effect.borrow_mut().push(previous.copied());
                    let value = source.get().expect("signal should be readable");
                    if should_panic_for_effect.replace(false) {
                        panic!("test previous effect panic");
                    }
                    Ok(value)
                },
                handler(scope),
            )
            .expect("previous effect should register");

        should_panic.set(true);
        let result = catch_unwind(AssertUnwindSafe(|| {
            set_source.set(2).expect("signal should be writable")
        }));
        assert!(result.is_err());

        set_source.set(3).expect("signal should be writable");
        assert_eq!(*seen.borrow(), vec![None, Some(1), Some(1)]);
    });
}

#[test]
fn callbacks_and_node_refs_are_scope_owned() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let called = std::rc::Rc::new(std::cell::Cell::new(0));
        let called_by_callback = called.clone();
        let callback = scope
            .callback(move |value: i32| {
                called_by_callback.set(value);
                Ok(())
            })
            .expect("callback should register");
        callback.invoke(11).expect("callback should be alive");
        callback.call(12).expect("callback should remain alive");
        assert_eq!(called.get(), 12);

        let node_ref = scope
            .node_ref::<String>()
            .expect("node ref should initialize");
        node_ref
            .load(String::from("node"))
            .expect("node ref should load");
        assert_eq!(
            node_ref
                .get()
                .expect("node ref should be readable")
                .as_deref(),
            Some("node")
        );
    });
}

#[test]
fn rx_macro_uses_explicit_scope_and_closure_reads() {
    let mut runtime = Runtime::new();
    let result = runtime.child(|scope| -> SilexResult<()> {
        let (count, set_count) = scope.signal(2i32).expect("signal should initialize");
        let value = rx!(scope; handler(scope); $count + 3);
        assert_eq!(value.get().expect("rx value should be readable"), 5);
        set_count.set(6).expect("signal should be writable");
        assert_eq!(value.get().expect("rx value should be readable"), 9);
        Ok(())
    });
    result
        .expect("child scope should initialize")
        .expect("rx should initialize");
}

#[test]
fn rx_macro_treats_parameterless_closures_as_derived_values() {
    let mut runtime = Runtime::new();
    let result = runtime.child(|scope| -> SilexResult<()> {
        let (count, set_count) = scope.signal(2i32).expect("signal should initialize");
        let value = rx!(scope; handler(scope); || $count + 3);
        assert_eq!(value.get().expect("rx value should be readable"), 5);
        set_count.set(6).expect("signal should be writable");
        assert_eq!(value.get().expect("rx value should be readable"), 9);
        Ok(())
    });
    result
        .expect("child scope should initialize")
        .expect("rx should initialize");
}

#[test]
fn rx_macro_parameterized_closures_return_fallible_callbacks() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let seen_in_callback = seen.clone();

    let result = runtime.child(|scope| -> SilexResult<()> {
        let callback = rx!(scope; handler(scope); move |value: i32| {
            seen_in_callback.set(value);
        });

        callback.invoke(7).expect("rx callback should be callable");
        Ok(())
    });
    result
        .expect("child scope should initialize")
        .expect("rx callback should register");

    assert_eq!(seen.get(), 7);
}

#[test]
fn callback_errors_preserve_user_and_runtime_variants() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let stale = Rc::new(Cell::new(false));
    let stale_after_cleanup = stale.clone();

    root.with_scope(|scope| {
        let callback = scope
            .callback(|_: ()| Err(SilexError::recoverable(SilexErrorKind::Framework(String::from("user error")))))
            .expect("callback should register");
        assert!(matches!(
            callback.invoke(()),
            Err(SilexError::Recoverable(SilexErrorKind::Framework(message))) if message == "user error"
        ));

        scope
            .on_cleanup(
                move || {
                    stale_after_cleanup.set(matches!(
                        callback.invoke(()),
                        Err(SilexError::Fatal(SilexErrorKind::Reactivity(ReactiveError::NoSuchNode)))
                    ));
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    root.dispose().expect("root should dispose");
    assert!(stale.get());
}

#[test]
fn stored_value_facade_and_rx_remain_available_during_final_cleanup() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let observed = Rc::new(RefCell::new(Vec::new()));

    root.with_scope(|scope| {
        let stored = scope.stored(1_i32).expect("stored value should initialize");
        let rx = stored.into_rx();
        let observed_in_cleanup = observed.clone();
        let scope_in_cleanup = scope;
        scope
            .on_cleanup(
                move || -> SilexResult<()> {
                    assert!(!scope_in_cleanup.is_active());
                    observed_in_cleanup.borrow_mut().push(
                        stored
                            .with(|value| *value)
                            .expect("stored value should be readable during cleanup"),
                    );
                    observed_in_cleanup.borrow_mut().push(
                        RxRead::with(&rx, |value| *value)
                            .expect("stored value rx should be readable during cleanup"),
                    );
                    stored
                        .update(|value| *value = 2)
                        .expect("stored value should be writable during cleanup");
                    observed_in_cleanup.borrow_mut().push(
                        RxRead::with(&rx, |value| *value)
                            .expect("stored value rx should observe the update"),
                    );
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    root.dispose().expect("root should dispose");
    assert_eq!(observed.borrow().as_slice(), &[1, 1, 2]);
}

#[test]
fn owned_scope_cleanup_can_update_a_facade_stored_value_before_dispose() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(0));

    let _ = runtime.child(|scope| {
        let stored = scope.stored(1_i32).expect("stored value should initialize");
        let owner = scope.owned_scope().expect("owner should initialize");
        let observed_in_cleanup = observed.clone();
        owner
            .on_cleanup(
                move || -> SilexResult<()> {
                    observed_in_cleanup.set(
                        stored
                            .update(|value| {
                                *value = 2;
                                *value
                            })
                            .expect("stored value should survive owner cleanup"),
                    );
                    Ok(())
                },
                handler(scope),
            )
            .expect("owner cleanup should register");

        owner.dispose().expect("owned scope should dispose");
        assert!(!owner.is_active());
    });

    assert_eq!(observed.get(), 2);
}

#[test]
fn child_scope_completes_lexically() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        scope
            .child(|child| {
                let (value, set_value) = child.signal(1i32).expect("signal should initialize");
                assert_eq!(value.get().expect("signal should be readable"), 1);
                set_value.set(2).expect("signal should be writable");
                assert_eq!(value.get().expect("signal should be readable"), 2);
            })
            .expect("child scope should initialize");
    });
}

#[test]
fn non_static_types_in_scoped_primitives() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let local_data = String::from("hello from scope");
        struct Borrowed<'a>(&'a str);

        let (sig, set_sig) = scope
            .signal(Borrowed(local_data.as_str()))
            .expect("signal should initialize");
        assert_eq!(
            sig.with(|b| b.0).expect("signal should be readable"),
            "hello from scope"
        );

        let updated_str = String::from("updated");
        set_sig
            .set(Borrowed(updated_str.as_str()))
            .expect("signal should be writable");
        assert_eq!(
            sig.with(|b| b.0).expect("signal should be readable"),
            "updated"
        );

        let stored = scope
            .stored(Borrowed(local_data.as_str()))
            .expect("stored value should initialize");
        assert_eq!(
            stored.with(|b| b.0).expect("stored should be readable"),
            "hello from scope"
        );
    });
}

#[test]
fn rx_default_creates_supported_wrappers_from_the_current_scope() {
    let mut runtime = Runtime::new();
    let _ = runtime.child(|scope| {
        let signal = <Signal<'_, i32> as RxDefault<'_>>::rx_default(scope)
            .expect("signal default should register");
        let explicit = <Signal<'_, String> as RxFrom<'_>>::rx_from(scope, "explicit")
            .expect("explicit signal should register");
        let read = <ReadSignal<'_, i32> as RxDefault<'_>>::rx_default(scope)
            .expect("read signal default should register");
        let rw = <RwSignal<'_, i32> as RxDefault<'_>>::rx_default(scope)
            .expect("rw signal default should register");
        let memo = <Memo<'_, i32> as RxDefault<'_>>::rx_default(scope)
            .expect("memo default should register");
        let stored = <StoredValue<'_, i32> as RxDefault<'_>>::rx_default(scope)
            .expect("stored default should register");
        let rx =
            <Rx<'_, i32> as RxDefault<'_>>::rx_default(scope).expect("rx default should register");
        let callback = <Callback<'_, i32> as RxDefault<'_>>::rx_default(scope)
            .expect("callback default should register");
        let node_ref = <NodeRef<'_, String> as RxDefault<'_>>::rx_default(scope)
            .expect("node ref default should register");

        assert_eq!(signal.get().expect("signal should be readable"), 0);
        assert_eq!(
            explicit.get().expect("signal should be readable"),
            "explicit"
        );
        assert_eq!(read.get().expect("read signal should be readable"), 0);
        assert_eq!(rw.get().expect("rw signal should be readable"), 0);
        assert_eq!(memo.get().expect("memo should be readable"), 0);
        assert_eq!(
            stored
                .with(|value| *value)
                .expect("stored should be readable"),
            0
        );
        assert_eq!(rx.get().expect("rx should be readable"), 0);
        callback
            .invoke(1)
            .expect("callback default should be callable");
        assert_eq!(node_ref.get(), Ok(None));
    });
}

#[test]
fn rx_default_nodes_keep_runtime_provenance() {
    let mut first = Runtime::new();
    let inputs = first
        .child(|scope| {
            let signal = <Signal<'_, i32> as RxDefault<'_>>::rx_default(scope)
                .expect("signal default should register");
            signal.into_rx().runtime_inputs()
        })
        .expect("child scope should initialize");

    let mut second = Runtime::new();
    let result = second
        .child(|scope| scope.validate_inputs(&inputs))
        .and_then(|result| result);

    assert!(matches!(
        result,
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::RuntimeMismatch,
        )))
    ));
}

#[test]
fn signal_source_and_other_handles_are_inactive_after_root_disposal() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("root should start");
    let stale = Rc::new(Cell::new(false));
    let stale_for_cleanup = stale.clone();

    root.with_scope(|scope| {
        let (read, _) = scope.signal(0_i32).expect("signal should initialize");
        let signal: Signal<'_, i32> = read.into();
        let callback = <Callback<'_, ()> as RxDefault<'_>>::rx_default(scope)
            .expect("callback default should register");
        let node_ref = <NodeRef<'_, String> as RxDefault<'_>>::rx_default(scope)
            .expect("node ref default should register");

        scope
            .on_cleanup(
                move || {
                    stale_for_cleanup.set(
                        matches!(
                            signal.get(),
                            Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                                ReactiveError::NoSuchNode
                            )))
                        ) && matches!(
                            callback.invoke(()),
                            Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                                ReactiveError::NoSuchNode,
                            )))
                        ) && matches!(node_ref.get(), Err(ReactiveError::NoSuchNode)),
                    );
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    root.dispose().expect("root disposal should succeed");
    assert!(stale.get());
}
