use silex_reactivity::{ErrorHandler, ReactiveError, Runtime, Scope};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {})
}

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
        let _effect = owner
            .effect(
                move || {
                    read.with(|value| {
                        assert!(*value >= 1);
                    });
                    runs_for_effect.set(runs_for_effect.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");
        let cleanups_for_owner = cleanups.clone();
        owner
            .on_cleanup(
                move || {
                    cleanups_for_owner.set(cleanups_for_owner.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");

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
        owner
            .effect(
                move || {
                    read.with(|value| {
                        assert!(*value >= 1);
                        assert_eq!(text.as_str(), "borrowed");
                    });
                    runs_for_effect.set(runs_for_effect.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");
        let child = owner.child();
        let child_cleanups = cleanups.clone();
        child
            .on_cleanup(
                move || {
                    child_cleanups.set(child_cleanups.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");

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

#[test]
fn fallible_owner_registration_rejects_inactive_scope() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let scope_for_cleanup = scope;
        let cleanup_error_handler = handler(scope);
        scope
            .on_cleanup(
                move || {
                    assert_eq!(
                        scope_for_cleanup.on_cleanup(|| Ok(()), cleanup_error_handler),
                        Err(ReactiveError::NoSuchNode)
                    );
                    assert!(matches!(
                        scope_for_cleanup.try_owned_scope(),
                        Err(ReactiveError::NoSuchNode)
                    ));
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    let mut root_runtime = Runtime::new();
    let root = root_runtime.run();
    let root_scope = root.scope();
    let owner = root_scope.try_owned_scope().expect("owner is active");
    assert!(owner.on_cleanup(|| Ok(()), handler(root_scope)).is_ok());
    owner.dispose();
    assert_eq!(
        owner.on_cleanup(|| Ok(()), handler(root_scope)),
        Err(ReactiveError::NoSuchNode)
    );
    assert!(matches!(owner.try_child(), Err(ReactiveError::NoSuchNode)));
    drop(owner);
    root.dispose().expect("root cleanup should succeed");
}

#[test]
fn fallible_cleanup_preserves_registration_order_during_dispose() {
    let mut runtime = Runtime::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let events_for_cleanup = events.clone();

    runtime.child(|scope| {
        let scope_for_cleanup = scope;
        let cleanup_error_handler = handler(scope);
        scope
            .on_cleanup(
                move || {
                    events_for_cleanup.borrow_mut().push("first");
                    assert_eq!(
                        scope_for_cleanup.on_cleanup(|| Ok(()), cleanup_error_handler),
                        Err(ReactiveError::NoSuchNode)
                    );
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    assert_eq!(events.borrow().as_slice(), ["first"]);
}
