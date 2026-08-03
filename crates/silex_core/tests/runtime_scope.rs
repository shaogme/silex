use silex_core::{Runtime, rx};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

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
fn lexical_effect_is_direct_and_tracks_dependencies() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (value, set_value) = scope.signal(1i32);
        let seen = Rc::new(Cell::new(0));
        let seen_for_effect = seen.clone();

        let effect = scope.effect(move || seen_for_effect.set(value.get()));

        assert!(effect.is_alive());
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

        let _effect = scope.effect_with_previous(move |previous: Option<i32>| {
            seen_for_effect.borrow_mut().push(previous);
            source.get()
        });

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

        let _effect = scope.effect_with_previous(move |previous: Option<i32>| {
            seen_for_effect.borrow_mut().push(previous);
            let value = source.get();
            if should_panic_for_effect.replace(false) {
                panic!("test previous effect panic");
            }
            value
        });

        should_panic.set(true);
        let result = catch_unwind(AssertUnwindSafe(|| set_source.set(2)));
        assert!(result.is_err());

        set_source.set(3);
        assert_eq!(*seen.borrow(), vec![None, Some(1), None]);
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
        assert!(callback.call(11));
        assert_eq!(called.get(), 11);

        let node_ref = scope.node_ref::<String>();
        assert!(node_ref.load(String::from("node")));
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
