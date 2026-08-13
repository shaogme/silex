use silex_core::{ReactiveError, Runtime, Scope, SilexError, SilexErrorKind};
use std::{cell::Cell, rc::Rc};

#[test]
fn high_level_root_uses_the_borrowed_scope_api() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.run().expect("root should start");

    {
        let scope = root.scope();
        let (value, set_value) = scope.signal(0i32).expect("signal should initialize");
        let seen_for_effect = seen.clone();
        let _effect = scope
            .effect(
                move || {
                    seen_for_effect.set(value.get()?);
                    Ok(())
                },
                scope
                    .error_handler(|_: SilexError| {})
                    .expect("error handler should register"),
            )
            .expect("effect should register");

        set_value.set(4).expect("signal should be writable");
        assert_eq!(seen.get(), 4);
    }

    root.dispose().expect("root disposal should succeed");
}

#[test]
fn high_level_scope_callbacks_receive_scope_values() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope: Scope<'_>| {
            let copied = scope;
            assert!(scope == copied);
        })
        .expect("child scope should initialize");

    let root = runtime.run().expect("root should start");
    root.with_scope(|scope: Scope<'_>| {
        let copied = scope;
        assert!(scope == copied);
    });
    root.dispose().expect("root disposal should succeed");
}

#[test]
fn high_level_try_run_reports_an_active_root() {
    let mut runtime = Runtime::new();
    let root = runtime.run().expect("first root should be created");

    assert!(matches!(
        runtime.run(),
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::RuntimeAlreadyRunning
        )))
    ));

    root.dispose().expect("root disposal should succeed");
}
