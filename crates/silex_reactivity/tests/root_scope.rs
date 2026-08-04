use silex_reactivity::{CompletionToken, Runtime, Scope};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn root_scope_uses_the_same_nodes_as_lexical_scope() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.run();

    {
        let scope = root.scope();
        let (value, set_value) = scope.signal(0i32);
        let seen_for_effect = seen.clone();
        let _effect = scope.effect(move || seen_for_effect.set(value.get()));

        set_value.set(3);
        assert_eq!(seen.get(), 3);
    }

    assert!(root.is_active());
    root.dispose().expect("root disposal should succeed");
}

#[test]
fn root_completion_is_invalidated_by_dispose() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.run();
    let token: CompletionToken<i32> = {
        let scope = root.scope();
        let seen_for_callback = seen.clone();
        scope.completion(move |value: i32| seen_for_callback.set(value))
    };

    assert!(token.submit(7));
    assert_eq!(seen.get(), 7);

    root.dispose().expect("root disposal should succeed");
    assert!(!token.submit(8));
    assert_eq!(seen.get(), 7);
}

#[test]
fn root_cleanup_runs_once_on_drop() {
    let cleaned = Rc::new(Cell::new(0));
    {
        let mut runtime = Runtime::new();
        let root = runtime.run();
        let scope = root.scope();
        let cleaned_for_scope = cleaned.clone();
        scope.on_cleanup(move || cleaned_for_scope.set(cleaned_for_scope.get() + 1));
    }
    assert_eq!(cleaned.get(), 1);
}

#[test]
fn root_cleanup_panic_is_reported_by_explicit_dispose() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    root.scope().on_cleanup(|| panic!("cleanup panic"));

    let result = root.dispose();
    assert!(result.is_err());
}

#[test]
fn runtime_rejects_run_while_root_is_active() {
    let mut runtime = Runtime::new();
    let root = runtime.run();

    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime.run();
    }));
    assert!(panic.is_err());

    root.dispose().expect("root disposal should succeed");
    let next_root = runtime.run();
    next_root.dispose().expect("second root should dispose");
}

#[test]
fn root_with_scope_keeps_non_static_payload_borrowed() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let text = String::from("root-local");

    root.with_scope(|scope| {
        let text_ref = &text;
        let stored = scope.stored(text_ref);
        assert_eq!(stored.with(|value| value.as_str()), "root-local");
    });

    root.dispose().expect("root disposal should succeed");
}

#[test]
fn scope_callbacks_receive_copyable_scope_values() {
    let mut runtime = Runtime::new();
    runtime.child(|scope: Scope<'_>| {
        let copied = scope;
        assert!(scope == copied);

        scope.child(|child: Scope<'_>| {
            let copied = child;
            assert!(child == copied);
        });
    });

    let root = runtime.run();
    root.with_scope(|scope: Scope<'_>| {
        let copied = scope;
        assert!(scope == copied);
    });
    root.dispose().expect("root disposal should succeed");
}
