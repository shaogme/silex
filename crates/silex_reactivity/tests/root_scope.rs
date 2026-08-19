#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::{
    CleanupFailure, CleanupPayloadKind, CloseError, ClosePhase, CloseSource, CloseTransaction,
    CompletionOnce, ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime, TransientScopeError,
    unwind_safe,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn handler<'scope>(scope: OwnerAccess<'scope>) -> ErrorHandlerToken<'scope, ()> {
    scope.error_handler(|_| {}).expect("handler registration")
}

#[test]
fn root_scope_uses_the_same_nodes_as_lexical_scope() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.owner().expect("runtime root creation");

    {
        let scope = root.access();
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

    assert!(root.is_active().expect("root active state"));
    root.close().expect("root disposal should succeed");
}

#[test]
fn close_transaction_preserves_phase_source_and_order() {
    let first = CloseError::from_panic(Box::new("child failure"));
    let second = CloseError::from_panic(Box::new("cleanup failure"));
    let mut transaction = CloseTransaction::new();
    transaction.push_error(ClosePhase::Child, CloseSource::Child, first);
    transaction.push_error(ClosePhase::Cleanup, CloseSource::Cleanup, second);

    let error = transaction
        .finish()
        .expect("transaction should contain errors");
    assert_eq!(error.entries().len(), 2);
    assert_eq!(error.entries()[0].phase(), ClosePhase::Child);
    assert_eq!(error.entries()[0].source(), CloseSource::Child);
    assert_eq!(error.entries()[1].phase(), ClosePhase::Cleanup);
    assert_eq!(error.entries()[1].source(), CloseSource::Cleanup);
    assert!(matches!(error.failures()[0], CleanupFailure::Panic(_)));
}

#[test]
fn root_completion_is_invalidated_by_dispose() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.owner().expect("runtime root creation");
    let token: CompletionOnce<i32, ()> = {
        let scope = root.access();
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

    root.close().expect("root disposal should succeed");
    assert!(!token.submit(8).expect("stale completion submit"));
    assert_eq!(seen.get(), 7);
}

#[test]
fn root_cleanup_runs_once_on_drop() {
    let cleaned = Rc::new(Cell::new(0));
    {
        let mut runtime = Runtime::new();
        let root = runtime.owner().expect("runtime root creation");
        let scope = root.access();
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
    let root = runtime.owner().expect("runtime root creation");

    root.with_access(|scope| {
        let stored = scope.stored(1_i32).expect("fallible reactive creation");
        let observed_in_cleanup = observed.clone();
        let scope_in_cleanup = scope;
        scope
            .on_cleanup(
                move || {
                    assert!(!scope_in_cleanup.is_active().expect("scope active state"));
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

    root.close().expect("root disposal should succeed");
    assert_eq!(observed.get(), 1);
}

#[test]
fn root_cleanup_panic_is_reported_by_explicit_dispose() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let scope = root.access();
    scope
        .on_cleanup(|| panic!("cleanup panic"), handler(scope))
        .expect("cleanup should register");

    let result = root.close();
    assert!(result.is_err());
}

#[test]
fn cleanup_error_exposes_stable_string_diagnostic_without_resuming_panic() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    root.with_access(|scope| {
        scope
            .on_cleanup(|| panic!("cleanup panic"), handler(scope))
            .expect("cleanup should register");
    });

    let error = root.close().expect_err("cleanup panic should be returned");
    assert_eq!(error.diagnostic().message(), "cleanup panic");
    assert_eq!(
        error.diagnostic().payload_kind(),
        CleanupPayloadKind::StaticStr
    );

    let diagnostic = error.into_diagnostic();
    assert_eq!(diagnostic.message(), "cleanup panic");
}

#[test]
fn transient_scope_preserves_close_error_classification() {
    let mut runtime = Runtime::new();
    let result = runtime.with_transient(|scope| {
        scope
            .on_cleanup(|| panic!("transient cleanup panic"), handler(scope))
            .expect("cleanup should register");
    });

    let TransientScopeError::Close(error) = result.expect_err("close failure should be returned")
    else {
        panic!("transient close failure was reclassified");
    };
    assert_eq!(error.diagnostic().message(), "transient cleanup panic");
    assert_eq!(
        error.diagnostic().payload_kind(),
        CleanupPayloadKind::StaticStr
    );
}

#[test]
fn cleanup_error_uses_unknown_diagnostic_for_non_string_payloads() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    root.with_access(|scope| {
        scope
            .on_cleanup(|| std::panic::panic_any(42_u32), handler(scope))
            .expect("cleanup should register");
    });

    let error = root.close().expect_err("cleanup panic should be returned");
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
    let root = runtime.owner().expect("runtime root creation");
    root.with_access(|scope| {
        scope
            .on_cleanup(|| std::panic::panic_any(PanicOnDrop), handler(scope))
            .expect("cleanup should register");
    });

    let error = root.close().expect_err("cleanup panic should be returned");
    let diagnostic = error.into_diagnostic();
    assert_eq!(diagnostic.payload_kind(), CleanupPayloadKind::Unknown);
}

#[test]
fn explicit_root_dispose_does_not_run_cleanup_again() {
    let cleaned = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    root.with_access(|scope| {
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

    root.close().expect("root disposal should succeed");
    assert_eq!(cleaned.get(), 1);
}

#[test]
fn direct_root_drop_is_best_effort_when_cleanup_panics() {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let mut runtime = Runtime::new();
        let root = runtime.owner().expect("runtime root creation");
        root.with_access(|scope| {
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
    let root = runtime.owner().expect("runtime root creation");

    assert!(matches!(
        runtime.owner(),
        Err(ReactiveError::RuntimeAlreadyRunning)
    ));

    root.close().expect("root disposal should succeed");
    let next_root = runtime.owner().expect("runtime root creation");
    next_root.close().expect("second root should dispose");
}

#[test]
fn try_run_reports_an_active_root_without_mutating_the_runtime_slot() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("first root should be created");

    assert!(matches!(
        runtime.owner(),
        Err(ReactiveError::RuntimeAlreadyRunning)
    ));

    drop(root);
    let next_root = runtime
        .owner()
        .expect("runtime should be reusable after root drop");
    next_root.close().expect("second root should dispose");
}

#[test]
fn root_with_scope_keeps_non_static_payload_borrowed() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let text = String::from("root-local");

    root.with_access(|scope| {
        let text_ref = &text;
        let stored = scope.stored(text_ref).expect("fallible reactive creation");
        assert_eq!(stored.with(|value| value.as_str()), Ok("root-local"));
    });

    root.close().expect("root disposal should succeed");
}

#[test]
fn scope_callbacks_receive_copyable_scope_values() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope: OwnerAccess<'_>| {
            let copied = scope;
            assert!(scope == copied);

            scope
                .with_transient(|child: OwnerAccess<'_>| {
                    let copied = child;
                    assert!(child == copied);
                })
                .expect("test operation should succeed");
        })
        .expect("test operation should succeed");

    let root = runtime.owner().expect("runtime root creation");
    root.with_access(|scope: OwnerAccess<'_>| {
        let copied = scope;
        assert!(scope == copied);
    });
    root.close().expect("root disposal should succeed");
}
