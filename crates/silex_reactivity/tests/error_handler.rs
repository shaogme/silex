#![allow(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic
)]

use silex_reactivity::Runtime;
use std::{cell::Cell, rc::Rc};

struct NonCopyError(String);

#[test]
fn error_handler_clone_keeps_scoped_callback_contract() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let label = String::from("scoped");
            let handler = scope
                .error_handler(move |error: &'static str| {
                    assert_eq!(error, label);
                })
                .expect("handler registration");
            let cloned = handler.clone();

            cloned.handle("scoped").expect("handler dispatch");
            scope
                .effect(|| Ok::<(), &'static str>(()), &handler)
                .expect("effect should initialize");
        })
        .expect("test operation should succeed");
}

#[test]
fn error_handler_ref_is_copy_without_copying_error_or_callback() {
    fn assert_copy<T: Copy>(_: T) {}

    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    runtime
        .with_transient(|scope| {
            let seen_in_handler = seen.clone();
            let token = scope
                .error_handler(move |error: NonCopyError| {
                    assert_eq!(error.0, "first");
                    seen_in_handler.set(seen_in_handler.get() + 1);
                })
                .expect("handler registration");
            let handler = token.view();
            assert_copy(handler);
            let copy = handler;

            handler
                .handle(NonCopyError(String::from("first")))
                .expect("handler dispatch");
            copy.handle(NonCopyError(String::from("first")))
                .expect("handler dispatch");
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 2);
}

#[test]
fn handlers_are_independent_and_can_dispatch_recursively() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    runtime
        .with_transient(|scope| {
            let seen_in_nested = seen.clone();
            let nested = scope
                .error_handler(move |error: &'static str| {
                    assert_eq!(error, "nested");
                    seen_in_nested.set(seen_in_nested.get() + 1);
                })
                .expect("handler registration");
            let seen_in_outer = seen.clone();
            let outer = scope
                .error_handler(move |error: &'static str| {
                    assert_eq!(error, "outer");
                    seen_in_outer.set(seen_in_outer.get() + 1);
                    nested.handle("nested").expect("nested handler dispatch");
                })
                .expect("handler registration");

            outer.handle("outer").expect("handler dispatch");
            assert_eq!(seen.get(), 2);
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 2);
}

#[test]
fn parent_handler_can_be_passed_to_a_child_scope() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let seen_in_handler = seen.clone();
            let token = scope
                .error_handler(move |_: &'static str| {
                    seen_in_handler.set(seen_in_handler.get() + 1);
                })
                .expect("handler registration");
            let handler = token.view();

            scope
                .with_transient(|child| {
                    child
                        .effect(|| Err::<(), &'static str>("child"), handler)
                        .expect_err("the child effect should return its initial error");
                    handler.handle("parent").expect("handler dispatch");
                })
                .expect("test operation should succeed");
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 1);
}

#[test]
fn handler_callback_can_read_and_update_signals() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("fallible reactive creation");
            let (value, set_value) = scope.signal(0_i32).expect("fallible reactive creation");
            let should_fail = Rc::new(Cell::new(false));
            let should_fail_in_effect = should_fail.clone();
            let observed_in_handler = observed.clone();
            let handler = scope
                .error_handler(move |_: &'static str| {
                    assert_eq!(value.get(), Ok(0));
                    set_value.set(1).expect("signal update");
                    observed_in_handler.set(value.get().expect("reactive read"));
                })
                .expect("handler registration");

            scope
                .effect(
                    move || {
                        source.get().expect("test operation should succeed");
                        if should_fail_in_effect.get() {
                            Err("deferred")
                        } else {
                            Ok(())
                        }
                    },
                    &handler,
                )
                .expect("effect should initialize");

            should_fail.set(true);
            set_source.set(1).expect("signal update");
        })
        .expect("test operation should succeed");

    assert_eq!(observed.get(), 1);
}

struct DropCapture(Rc<Cell<usize>>);

impl Drop for DropCapture {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn dropping_the_last_token_retires_the_callback_immediately() {
    let mut runtime = Runtime::new();
    let drops = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let capture = DropCapture(drops.clone());
            let token = scope
                .error_handler(move |_: ()| {
                    let _ = &capture;
                })
                .expect("handler registration");
            let view = token.view();

            drop(token);
            assert_eq!(drops.get(), 1);
            assert!(view.handle(()).is_err());
        })
        .expect("test operation should succeed");
}

#[test]
fn computation_lease_survives_token_drop_but_view_becomes_stale() {
    let mut runtime = Runtime::new();
    let drops = Rc::new(Cell::new(0));
    let handled = Rc::new(Cell::new(0));
    let should_fail = Rc::new(Cell::new(false));

    runtime
        .with_transient(|scope| {
            let (source, set_source) = scope.signal(0_i32).expect("signal registration");
            let capture = DropCapture(drops.clone());
            let handled_in_callback = handled.clone();
            let should_fail_in_callback = should_fail.clone();
            let token = scope
                .error_handler(move |_: &'static str| {
                    let _ = &capture;
                    handled_in_callback.set(handled_in_callback.get() + 1);
                })
                .expect("handler registration");
            let view = token.view();
            let effect = scope
                .effect(
                    move || {
                        source.get().expect("signal read");
                        if should_fail_in_callback.get() {
                            Err("deferred")
                        } else {
                            Ok(())
                        }
                    },
                    view,
                )
                .expect("effect registration");

            drop(token);
            assert!(view.handle("stale").is_err());
            should_fail.set(true);
            set_source.set(1).expect("signal update");
            assert_eq!(handled.get(), 1);
            effect.stop().expect("effect disposal");
            assert_eq!(drops.get(), 1);
        })
        .expect("test operation should succeed");
}

#[test]
fn cloned_tokens_share_the_registration_owner() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let token = scope
                .error_handler(|_: ()| {})
                .expect("handler registration");
            let clone = token.clone();
            let view = token.view();

            drop(token);
            view.handle(()).expect("clone should keep handler active");
            drop(clone);
            assert!(view.handle(()).is_err());
        })
        .expect("test operation should succeed");
}

#[test]
fn stale_view_cannot_dispatch_a_reused_registration_slot() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let old = scope
                .error_handler(|_: ()| {})
                .expect("old handler registration");
            let stale = old.view();
            drop(old);

            let seen_in_new_handler = seen.clone();
            let current = scope
                .error_handler(move |_: ()| {
                    seen_in_new_handler.set(seen_in_new_handler.get() + 1);
                })
                .expect("new handler registration");
            current.handle(()).expect("current handler dispatch");
            assert!(stale.handle(()).is_err());
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 1);
}

#[test]
fn handler_anchor_survives_caller_token_release() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime
        .with_transient(|scope| {
            let seen_in_handler = seen.clone();
            let token = scope
                .error_handler(move |_: ()| {
                    seen_in_handler.set(seen_in_handler.get() + 1);
                })
                .expect("handler registration");
            let anchor = token.view().anchor().expect("handler anchor");

            drop(token);
            anchor.view().handle(()).expect("anchor dispatch");
        })
        .expect("test operation should succeed");

    assert_eq!(seen.get(), 1);
}

#[test]
#[cfg(feature = "test-support")]
fn retired_handlers_are_excluded_from_active_snapshots() {
    let mut runtime = Runtime::new();

    runtime
        .with_transient(|scope| {
            let token = scope
                .error_handler(|_: ()| {})
                .expect("handler registration");
            let view = token.view();
            assert_eq!(
                scope.runtime_snapshot().expect("runtime snapshot").handlers,
                1
            );

            drop(token);
            assert_eq!(
                scope.runtime_snapshot().expect("runtime snapshot").handlers,
                0
            );
            assert!(view.handle(()).is_err());

            let replacement = scope
                .error_handler(|_: ()| {})
                .expect("replacement handler registration");
            assert_eq!(
                scope.runtime_snapshot().expect("runtime snapshot").handlers,
                1
            );
            drop(replacement);
            assert_eq!(
                scope.runtime_snapshot().expect("runtime snapshot").handlers,
                0
            );
        })
        .expect("test operation should succeed");
}
