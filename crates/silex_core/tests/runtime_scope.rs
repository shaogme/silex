use silex_core::{
    Callback, ErrorHandler, Memo, NodeRef, ReactiveError, ReadSignal, Runtime, RwSignal, Rx,
    RxDefault, RxFrom, Signal, SilexError, StoredValue, rx,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>() -> ErrorHandler<'scope, SilexError> {
    ErrorHandler::new(|_| {})
}

#[test]
fn scoped_primitives_propagate_without_raw_handles() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (count, set_count) = scope.signal(1i32);
        let doubled = scope.promote(count).map(|value| value * 2);
        let memo = scope.memo_from(doubled.runtime_inputs(), move |_| doubled.get() + 1);

        assert_eq!(doubled.get(), 2);
        assert_eq!(memo.get(), 3);
        set_count.set(4);
        assert_eq!(doubled.get(), 8);
        assert_eq!(memo.get(), 9);

        let stored = scope.stored(String::from("stored"));
        assert!(stored.with(|value| value == "stored"));
        stored.update(|value| value.push_str(" value"));
        assert!(stored.with(|value| value == "stored value"));
    });
}

#[test]
fn scope_memo_and_derived_have_no_input_convenience_entries() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, set_source) = scope.signal(2i32);
        let derived = scope.derived(move || source.get() * 2);
        let memo = scope.memo(move |_| derived.get() + 1);

        assert_eq!(derived.get(), 4);
        assert_eq!(memo.get(), 5);
        set_source.set(6);
        assert_eq!(derived.get(), 12);
        assert_eq!(memo.get(), 13);
    });
}

#[test]
fn owned_scope_exposes_an_owner_bound_effect_only() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let runs = Rc::new(Cell::new(0));
        let runs_for_effect = runs.clone();
        let owner = scope.owned_scope();
        let _effect = owner
            .effect(
                move || {
                    let _ = source.get();
                    runs_for_effect.set(runs_for_effect.get() + 1);
                    Ok(())
                },
                handler(),
            )
            .expect("owned effect should register");

        set_source.set(2);
        assert_eq!(runs.get(), 2);
        owner.dispose();
        set_source.set(3);
        assert_eq!(runs.get(), 2);
    });
}

#[test]
fn lexical_effect_is_direct_and_tracks_dependencies() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(1i32);
        let seen = Rc::new(Cell::new(0));
        let seen_for_effect = seen.clone();

        let _effect = scope
            .effect(
                move || {
                    seen_for_effect.set(value.get());
                    Ok(())
                },
                handler(),
            )
            .expect("effect should register");

        assert_eq!(seen.get(), 1);
        set_value.set(4);
        assert_eq!(seen.get(), 4);
    });
}

#[test]
fn previous_effect_commits_the_last_returned_value() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_for_effect = seen.clone();

        let _effect = scope
            .effect_with_previous(
                move |previous: Option<&i32>| {
                    seen_for_effect.borrow_mut().push(previous.copied());
                    Ok(source.get())
                },
                handler(),
            )
            .expect("previous effect should register");

        set_source.set(2);
        set_source.set(3);
        assert_eq!(*seen.borrow(), vec![None, Some(1), Some(2)]);
    });
}

#[test]
fn previous_effect_resets_after_a_panicking_run() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let seen_for_effect = seen.clone();
        let should_panic = Rc::new(Cell::new(false));
        let should_panic_for_effect = should_panic.clone();

        let _effect = scope
            .effect_with_previous(
                move |previous: Option<&i32>| {
                    seen_for_effect.borrow_mut().push(previous.copied());
                    let value = source.get();
                    if should_panic_for_effect.replace(false) {
                        panic!("test previous effect panic");
                    }
                    Ok(value)
                },
                handler(),
            )
            .expect("previous effect should register");

        should_panic.set(true);
        let result = catch_unwind(AssertUnwindSafe(|| set_source.set(2)));
        assert!(result.is_err());

        set_source.set(3);
        assert_eq!(*seen.borrow(), vec![None, Some(1), Some(1)]);
    });
}

#[test]
fn callbacks_and_node_refs_are_scope_owned() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let called = std::rc::Rc::new(std::cell::Cell::new(0));
        let called_by_callback = called.clone();
        let callback = scope.callback(move |value: i32| {
            called_by_callback.set(value);
        });
        callback.invoke(11).expect("callback should be alive");
        callback.call(12).expect("callback should remain alive");
        assert_eq!(called.get(), 12);

        let node_ref = scope.node_ref::<String>();
        node_ref.load(String::from("node"));
        assert_eq!(node_ref.get().as_deref(), Some("node"));
    });
}

#[test]
fn rx_macro_uses_explicit_scope_and_closure_reads() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (count, set_count) = scope.signal(2i32);
        let value = rx!(scope; $count + 3);
        assert_eq!(value.get(), 5);
        set_count.set(6);
        assert_eq!(value.get(), 9);
    });
}

#[test]
fn rx_macro_treats_parameterless_closures_as_derived_values() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (count, set_count) = scope.signal(2i32);
        let value = rx!(scope; || $count + 3);
        assert_eq!(value.get(), 5);
        set_count.set(6);
        assert_eq!(value.get(), 9);
    });
}

#[test]
fn child_scope_completes_lexically() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        scope.child(|child| {
            let (value, set_value) = child.signal(1i32);
            assert_eq!(value.get(), 1);
            set_value.set(2);
            assert_eq!(value.get(), 2);
        });
    });
}

#[test]
fn non_static_types_in_scoped_primitives() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let local_data = String::from("hello from scope");
        struct Borrowed<'a>(&'a str);

        let (sig, set_sig) = scope.signal(Borrowed(local_data.as_str()));
        assert_eq!(sig.with(|b| b.0), "hello from scope");

        let updated_str = String::from("updated");
        set_sig.set(Borrowed(updated_str.as_str()));
        assert_eq!(sig.with(|b| b.0), "updated");

        let stored = scope.stored(Borrowed(local_data.as_str()));
        assert_eq!(stored.with(|b| b.0), "hello from scope");
    });
}

#[test]
fn rx_default_creates_supported_wrappers_from_the_current_scope() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let signal = <Signal<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let explicit = <Signal<'_, String> as RxFrom<'_>>::rx_from(scope, "explicit");
        let read = <ReadSignal<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let rw = <RwSignal<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let memo = <Memo<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let stored = <StoredValue<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let rx = <Rx<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let callback = <Callback<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let node_ref = <NodeRef<'_, String> as RxDefault<'_>>::rx_default(scope);

        assert_eq!(signal.get(), 0);
        assert_eq!(explicit.get(), "explicit");
        assert_eq!(read.get(), 0);
        assert_eq!(rw.get(), 0);
        assert_eq!(memo.get(), 0);
        assert_eq!(stored.with(|value| *value), 0);
        assert_eq!(rx.get(), 0);
        assert!(callback.invoke(1).is_ok());
        assert_eq!(node_ref.try_get(), Ok(None));
    });
}

#[test]
fn rx_default_nodes_keep_runtime_provenance() {
    let mut first = Runtime::new();
    let inputs = first.child(|scope| {
        let signal = <Signal<'_, i32> as RxDefault<'_>>::rx_default(scope);
        signal.into_rx().runtime_inputs()
    });

    let mut second = Runtime::new();
    let result = second.child(|scope| scope.try_validate_inputs(&inputs));

    assert!(matches!(
        result,
        Err(silex_core::SilexError::Reactivity(
            ReactiveError::RuntimeMismatch
        ))
    ));
}

#[test]
fn rx_default_handles_are_inactive_after_root_disposal() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let stale = Rc::new(Cell::new(false));
    let stale_for_cleanup = stale.clone();

    root.with_scope(|scope| {
        let signal = <Signal<'_, i32> as RxDefault<'_>>::rx_default(scope);
        let callback = <Callback<'_, ()> as RxDefault<'_>>::rx_default(scope);
        let node_ref = <NodeRef<'_, String> as RxDefault<'_>>::rx_default(scope);

        scope
            .on_cleanup(
                move || {
                    stale_for_cleanup.set(
                        matches!(signal.try_get(), Err(ReactiveError::NoSuchNode))
                            && matches!(
                                callback.invoke(()),
                                Err(silex_core::SilexError::Reactivity(
                                    ReactiveError::NoSuchNode
                                ))
                            )
                            && matches!(node_ref.try_get(), Err(ReactiveError::NoSuchNode)),
                    );
                    Ok(())
                },
                handler(),
            )
            .expect("cleanup should register");
    });

    root.dispose().expect("root disposal should succeed");
    assert!(stale.get());
}
