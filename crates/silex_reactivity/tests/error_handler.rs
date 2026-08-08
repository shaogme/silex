use silex_reactivity::Runtime;
use std::{cell::Cell, rc::Rc};

struct NonCopyError(String);

#[test]
fn error_handler_clone_keeps_scoped_callback_contract() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let label = String::from("scoped");
        let handler = scope.error_handler(move |error: &'static str| {
            assert_eq!(error, label);
        });
        let cloned = handler;

        cloned.handle("scoped");
        scope
            .effect(|| Ok::<(), &'static str>(()), handler)
            .expect("effect should initialize");
    });
}

#[test]
fn error_handler_is_copy_without_copying_error_or_callback() {
    fn assert_copy<T: Copy>(_: T) {}

    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    runtime.child(|scope| {
        let seen_in_handler = seen.clone();
        let handler = scope.error_handler(move |error: NonCopyError| {
            assert_eq!(error.0, "first");
            seen_in_handler.set(seen_in_handler.get() + 1);
        });
        assert_copy(handler);
        let copy = handler;

        handler.handle(NonCopyError(String::from("first")));
        copy.handle(NonCopyError(String::from("first")));
    });

    assert_eq!(seen.get(), 2);
}

#[test]
fn handlers_are_independent_and_can_dispatch_recursively() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    runtime.child(|scope| {
        let seen_in_nested = seen.clone();
        let nested = scope.error_handler(move |error: &'static str| {
            assert_eq!(error, "nested");
            seen_in_nested.set(seen_in_nested.get() + 1);
        });
        let seen_in_outer = seen.clone();
        let outer = scope.error_handler(move |error: &'static str| {
            assert_eq!(error, "outer");
            seen_in_outer.set(seen_in_outer.get() + 1);
            nested.handle("nested");
        });

        outer.handle("outer");
        assert_eq!(seen.get(), 2);
    });

    assert_eq!(seen.get(), 2);
}

#[test]
fn parent_handler_can_be_passed_to_a_child_scope() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let seen_in_handler = seen.clone();
        let handler = scope.error_handler(move |_: &'static str| {
            seen_in_handler.set(seen_in_handler.get() + 1);
        });

        scope.child(|child| {
            child
                .effect(|| Err::<(), &'static str>("child"), handler)
                .expect_err("the child effect should return its initial error");
            handler.handle("parent");
        });
    });

    assert_eq!(seen.get(), 1);
}

#[test]
fn handler_callback_can_read_and_update_signals() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0_i32);
        let (value, set_value) = scope.signal(0_i32);
        let should_fail = Rc::new(Cell::new(false));
        let should_fail_in_effect = should_fail.clone();
        let observed_in_handler = observed.clone();
        let handler = scope.error_handler(move |_: &'static str| {
            assert_eq!(value.get(), 0);
            set_value.set(1);
            observed_in_handler.set(value.get());
        });

        scope
            .effect(
                move || {
                    let _ = source.get();
                    if should_fail_in_effect.get() {
                        Err("deferred")
                    } else {
                        Ok(())
                    }
                },
                handler,
            )
            .expect("effect should initialize");

        should_fail.set(true);
        set_source.set(1);
    });

    assert_eq!(observed.get(), 1);
}
