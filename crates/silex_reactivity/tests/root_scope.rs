use silex_reactivity::{
    CleanupPayloadKind, CompletionOnce, ErrorHandler, ReactiveError, Runtime, Scope, unwind_safe,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn root_scope_uses_the_same_nodes_as_lexical_scope() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.run().expect("runtime root creation");

    {
        let scope = root.scope();
        let (value, set_value) = scope.signal(0i32).expect("fallible reactive creation");
        let seen_for_effect = seen.clone();
        let _effect = scope
            .effect(
                move || {
                    seen_for_effect.set(value.get().expect("reactive read"));
                    Ok(())
                },
                handler(scope),
            )
            .expect("effect should initialize");

        set_value.set(3).expect("signal update");
        assert_eq!(seen.get(), 3);
    }

    assert!(root.is_active());
    root.dispose().expect("root disposal should succeed");
}

#[test]
fn root_completion_is_invalidated_by_dispose() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.run().expect("runtime root creation");
    let token: CompletionOnce<i32, ()> = {
        let scope = root.scope();
        let seen_for_callback = seen.clone();
        scope
            .completion_once(unwind_safe(move |value: i32| {
                seen_for_callback.set(value);
                Ok::<(), ()>(())
            }))
            .expect("completion registration")
    };

    assert!(token.submit(7).expect("completion submit"));
    assert_eq!(seen.get(), 7);

    root.dispose().expect("root disposal should succeed");
    assert!(!token.submit(8).expect("stale completion submit"));
    assert_eq!(seen.get(), 7);
}

#[test]
fn root_cleanup_runs_once_on_drop() {
    let cleaned = Rc::new(Cell::new(0));
    {
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("runtime root creation");
        let scope = root.scope();
        let cleaned_for_scope = cleaned.clone();
        scope
            .on_cleanup(
                move || {
                    cleaned_for_scope.set(cleaned_for_scope.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    }
    assert_eq!(cleaned.get(), 1);
}

#[test]
fn root_final_cleanup_can_update_a_stored_value_before_drop() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(0));
    let root = runtime.run().expect("runtime root creation");

    root.with_scope(|scope| {
        let stored = scope.stored(1_i32).expect("fallible reactive creation");
        let observed_in_cleanup = observed.clone();
        let scope_in_cleanup = scope;
        scope
            .on_cleanup(
                move || {
                    assert!(!scope_in_cleanup.is_active());
                    observed_in_cleanup.set(
                        stored
                            .with(|value| *value)
                            .expect("stored value should survive until cleanup"),
                    );
                    stored
                        .update(|value| *value = 2)
                        .expect("stored value should be writable during cleanup");
                    assert_eq!(stored.with(|value| *value), Ok(2));
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    root.dispose().expect("root disposal should succeed");
    assert_eq!(observed.get(), 1);
}

#[test]
fn root_cleanup_panic_is_reported_by_explicit_dispose() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");
    let scope = root.scope();
    scope
        .on_cleanup(|| panic!("cleanup panic"), handler(scope))
        .expect("cleanup should register");

    let result = root.dispose();
    assert!(result.is_err());
}

#[test]
fn cleanup_error_exposes_stable_string_diagnostic_without_resuming_panic() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");
    root.with_scope(|scope| {
        scope
            .on_cleanup(|| panic!("cleanup panic"), handler(scope))
            .expect("cleanup should register");
    });

    let error = root
        .dispose()
        .expect_err("cleanup panic should be returned");
    assert_eq!(error.diagnostic().message(), "cleanup panic");
    assert_eq!(
        error.diagnostic().payload_kind(),
        CleanupPayloadKind::StaticStr
    );

    let diagnostic = error.into_diagnostic();
    assert_eq!(diagnostic.message(), "cleanup panic");
}

#[test]
fn cleanup_error_uses_unknown_diagnostic_for_non_string_payloads() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");
    root.with_scope(|scope| {
        scope
            .on_cleanup(|| std::panic::panic_any(42_u32), handler(scope))
            .expect("cleanup should register");
    });

    let error = root
        .dispose()
        .expect_err("cleanup panic should be returned");
    assert_eq!(
        error.diagnostic().payload_kind(),
        CleanupPayloadKind::Unknown
    );
    assert_eq!(
        error.diagnostic().message(),
        "unknown cleanup panic payload"
    );
}

#[test]
fn diagnostic_conversion_does_not_resume_a_payload_drop_panic() {
    struct PanicOnDrop;

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            panic!("payload drop panic");
        }
    }

    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");
    root.with_scope(|scope| {
        scope
            .on_cleanup(|| std::panic::panic_any(PanicOnDrop), handler(scope))
            .expect("cleanup should register");
    });

    let error = root
        .dispose()
        .expect_err("cleanup panic should be returned");
    let diagnostic = error.into_diagnostic();
    assert_eq!(diagnostic.payload_kind(), CleanupPayloadKind::Unknown);
}

#[test]
fn explicit_root_dispose_does_not_run_cleanup_again() {
    let cleaned = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");
    root.with_scope(|scope| {
        let cleaned = cleaned.clone();
        scope
            .on_cleanup(
                move || {
                    cleaned.set(cleaned.get() + 1);
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    root.dispose().expect("root disposal should succeed");
    assert_eq!(cleaned.get(), 1);
}

#[test]
fn direct_root_drop_is_best_effort_when_cleanup_panics() {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let mut runtime = Runtime::new();
        let root = runtime.run().expect("runtime root creation");
        root.with_scope(|scope| {
            scope
                .on_cleanup(|| panic!("direct drop panic"), handler(scope))
                .expect("cleanup should register");
        });
        drop(root);
    }));

    assert!(panic.is_ok());
}

#[test]
fn runtime_rejects_run_while_root_is_active() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");

    assert!(matches!(
        runtime.run(),
        Err(ReactiveError::RuntimeAlreadyRunning)
    ));

    root.dispose().expect("root disposal should succeed");
    let next_root = runtime.run().expect("runtime root creation");
    next_root.dispose().expect("second root should dispose");
}

#[test]
fn try_run_reports_an_active_root_without_mutating_the_runtime_slot() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("first root should be created");

    assert!(matches!(
        runtime.run(),
        Err(ReactiveError::RuntimeAlreadyRunning)
    ));

    drop(root);
    let next_root = runtime
        .run()
        .expect("runtime should be reusable after root drop");
    next_root.dispose().expect("second root should dispose");
}

#[test]
fn root_with_scope_keeps_non_static_payload_borrowed() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("runtime root creation");
    let text = String::from("root-local");

    root.with_scope(|scope| {
        let text_ref = &text;
        let stored = scope.stored(text_ref).expect("fallible reactive creation");
        assert_eq!(stored.with(|value| value.as_str()), Ok("root-local"));
    });

    root.dispose().expect("root disposal should succeed");
}

#[test]
fn scope_callbacks_receive_copyable_scope_values() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope: Scope<'_>| {
            let copied = scope;
            assert!(scope == copied);

            scope
                .child(|child: Scope<'_>| {
                    let copied = child;
                    assert!(child == copied);
                })
                .expect("test operation should succeed");
        })
        .expect("test operation should succeed");

    let root = runtime.run().expect("runtime root creation");
    root.with_scope(|scope: Scope<'_>| {
        let copied = scope;
        assert!(scope == copied);
    });
    root.dispose().expect("root disposal should succeed");
}
