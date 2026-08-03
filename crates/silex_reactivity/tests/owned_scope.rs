use silex_reactivity::Runtime;
use std::{cell::Cell, rc::Rc};

#[test]
fn owned_scope_keeps_effects_until_explicit_dispose() {
    let mut runtime = Runtime::new();
    let mut values = None;
    let root = runtime.run(|scope| {
        let (read, write) = scope.signal(1i32);
        let runs = Rc::new(Cell::new(0));
        let cleanups = Rc::new(Cell::new(0));
        let owner = scope.owned_scope();

        let runs_for_effect = runs.clone();
        owner.effect(move || {
            read.with(|value| {
                assert!(*value >= 1);
            });
            runs_for_effect.set(runs_for_effect.get() + 1);
        });
        let cleanups_for_owner = cleanups.clone();
        owner.on_cleanup(move || {
            cleanups_for_owner.set(cleanups_for_owner.get() + 1);
        });

        values = Some((write, owner, runs, cleanups));
    });

    let (write, owner, runs, cleanups) = values.expect("root callback created owner state");
    assert_eq!(runs.get(), 1);
    write.set(2);
    assert_eq!(runs.get(), 2);

    owner.dispose();
    assert!(!owner.is_active());
    assert_eq!(cleanups.get(), 1);
    write.set(3);
    assert_eq!(runs.get(), 2);
    owner.dispose();
    assert_eq!(cleanups.get(), 1);

    drop(root);
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
