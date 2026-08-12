use silex_reactivity::{ErrorHandler, ReactiveError, Runtime, Scope, unwind_safe};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn owned_scope_keeps_effects_until_explicit_dispose() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");
    {
        let scope = root.scope();
        let (read, write) = scope.signal(1i32).expect("fallible reactive creation");
        let runs = Rc::new(Cell::new(0));
        let cleanups = Rc::new(Cell::new(0));
        let owner = scope.owned_scope().expect("fallible reactive creation");

        let runs_for_effect = runs.clone();
        let _effect = owner
            .effect(
                move || {
                    read.with(|value| {
                        assert!(*value >= 1);
                    })
                    .expect("test operation should succeed");
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
        write.set(2).expect("signal update");
        assert_eq!(runs.get(), 2);

        owner.dispose().expect("owner disposal");
        assert!(!owner.is_active());
        assert_eq!(cleanups.get(), 1);
        write.set(3).expect("signal update");
        assert_eq!(runs.get(), 2);
        owner.dispose().expect("owner disposal");
        assert_eq!(cleanups.get(), 1);
    }

    root.dispose().expect("root disposal should succeed");
}

#[test]
fn owned_scope_cleanup_can_release_captured_stored_value() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let stored = scope.stored(1_i32).expect("fallible reactive creation");
            let owner = scope.owned_scope().expect("fallible reactive creation");
            let observed_in_cleanup = observed.clone();
            owner
                .on_cleanup(
                    move || {
                        observed_in_cleanup.set(
                            stored
                                .update(|value| {
                                    *value = 2;
                                    *value
                                })
                                .expect("captured stored value should be available"),
                        );
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("owner cleanup should register");

            owner.dispose().expect("owner disposal");
            assert!(!owner.is_active());
        })
        .expect("test operation should succeed");

    assert_eq!(observed.get(), 2);
}

#[test]
fn lexical_owned_scope_supports_borrowed_callbacks_and_nested_dispose() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let text = String::from("borrowed");
            let (read, write) = scope.signal(1i32).expect("fallible reactive creation");
            let owner = scope.owned_scope().expect("fallible reactive creation");
            let runs = Rc::new(Cell::new(0));
            let cleanups = Rc::new(Cell::new(0));

            let runs_for_effect = runs.clone();
            owner
                .effect(
                    move || {
                        read.with(|value| {
                            assert!(*value >= 1);
                            assert_eq!(text.as_str(), "borrowed");
                        })
                        .expect("test operation should succeed");
                        runs_for_effect.set(runs_for_effect.get() + 1);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("effect should initialize");
            let child = owner.child().expect("child scope creation");
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

            write.set(2).expect("signal update");
            assert_eq!(runs.get(), 2);
            child.dispose().expect("child disposal");
            owner.dispose().expect("owner disposal");
            assert_eq!(cleanups.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn owned_scope_completion_can_capture_scope_local_data() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .child(|scope| {
            let owner = scope.owned_scope().expect("fallible reactive creation");
            let local = String::from("owned");
            let seen_in_callback = seen.clone();
            let token = owner
                .completion_once(unwind_safe(move |value: i32| {
                    assert_eq!(local, "owned");
                    seen_in_callback.set(value);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            assert!(token.submit(9).expect("completion submit"));
            owner.dispose().expect("owner disposal");
            assert!(!token.submit(10).expect("stale completion submit"));
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 9);
}

#[test]
fn fallible_owner_registration_rejects_inactive_scope() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
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
                            scope_for_cleanup.owned_scope(),
                            Err(ReactiveError::NoSuchNode)
                        ));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("test operation should succeed");

    let mut root_runtime = Runtime::new();
    let root = root_runtime.run().expect("runtime root creation");
    let root_scope = root.scope();
    let owner = root_scope.owned_scope().expect("owner is active");
    assert!(owner.on_cleanup(|| Ok(()), handler(root_scope)).is_ok());
    owner.dispose().expect("owner disposal");
    assert_eq!(
        owner.on_cleanup(|| Ok(()), handler(root_scope)),
        Err(ReactiveError::NoSuchNode)
    );
    assert!(matches!(owner.child(), Err(ReactiveError::NoSuchNode)));
    drop(owner);
    root.dispose().expect("root cleanup should succeed");
}

#[test]
fn fallible_cleanup_preserves_registration_order_during_dispose() {
    let mut runtime = Runtime::new();
    let events = Rc::new(RefCell::new(Vec::new()));
    let events_for_cleanup = events.clone();

    runtime
        .child(|scope| {
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
        })
        .expect("test operation should succeed");

    assert_eq!(events.borrow().as_slice(), ["first"]);
}
