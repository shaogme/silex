use silex_reactivity::Runtime;
use std::{cell::Cell, rc::Rc};

#[test]
fn owned_scope_keeps_effects_until_explicit_dispose() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    {
        let scope = root.scope();
        let (read, write) = scope.signal(1i32);
        let runs = Rc::new(Cell::new(0));
        let cleanups = Rc::new(Cell::new(0));
        let owner = scope.owned_scope();

        let runs_for_effect = runs.clone();
        let effect = owner.effect(move || {
            read.with(|value| {
                assert!(*value >= 1);
            });
            runs_for_effect.set(runs_for_effect.get() + 1);
        });
        assert!(effect.is_alive());
        let cleanups_for_owner = cleanups.clone();
        owner.on_cleanup(move || {
            cleanups_for_owner.set(cleanups_for_owner.get() + 1);
        });

        assert_eq!(runs.get(), 1);
        write.set(2);
        assert_eq!(runs.get(), 2);

        owner.dispose();
        assert!(!owner.is_active());
        assert!(!effect.is_alive());
        assert_eq!(cleanups.get(), 1);
        write.set(3);
        assert_eq!(runs.get(), 2);
        owner.dispose();
        assert_eq!(cleanups.get(), 1);
    }

    root.dispose().expect("root disposal should succeed");
}

#[test]
fn lexical_owned_scope_supports_borrowed_callbacks_and_nested_dispose() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let text = String::from("borrowed");
        let (read, write) = scope.signal(1i32);
        let owner = scope.owned_scope();
        let runs = Rc::new(Cell::new(0));
        let cleanups = Rc::new(Cell::new(0));

        let runs_for_effect = runs.clone();
        owner.effect(move || {
            read.with(|value| {
                assert!(*value >= 1);
                assert_eq!(text.as_str(), "borrowed");
            });
            runs_for_effect.set(runs_for_effect.get() + 1);
        });
        let child = owner.child();
        let child_cleanups = cleanups.clone();
        child.on_cleanup(move || {
            child_cleanups.set(child_cleanups.get() + 1);
        });

        write.set(2);
        assert_eq!(runs.get(), 2);
        child.dispose();
        owner.dispose();
        assert_eq!(cleanups.get(), 1);
    });
}

#[test]
fn owned_scope_completion_can_capture_scope_local_data() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let owner = scope.owned_scope();
        let local = String::from("owned");
        let seen_in_callback = seen.clone();
        let token = owner.completion_once(move |value: i32| {
            assert_eq!(local, "owned");
            seen_in_callback.set(value);
        });
        assert!(token.submit(9));
        owner.dispose();
        assert!(!token.submit(10));
    });

    assert_eq!(seen.get(), 9);
}
