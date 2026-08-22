use silex_core::{
    EffectPhase, OwnerAccess, ReactiveError, Runtime, SilexError, SilexErrorKind, traits::RxGet,
};
use std::{cell::Cell, rc::Rc};

#[test]
fn high_level_root_uses_the_borrowed_scope_api() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let root = runtime.owner().expect("root should start");

    {
        let owner = root.access();
        let value = owner.signal(0i32).expect("signal should initialize");
        let seen_for_effect = seen.clone();
        let _effect = owner
            .effect(
                EffectPhase::Normal,
                move || {
                    seen_for_effect.set(value.get()?);
                    Ok(())
                },
                owner
                    .error_handler(|_: SilexError| {})
                    .expect("error handler should register")
                    .view(),
            )
            .expect("effect should register");

        value.set(4).expect("signal should be writable");
        assert_eq!(seen.get(), 4);
    }

    root.close().expect("root disposal should succeed");
}

#[test]
fn high_level_scope_callbacks_receive_scope_values() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner: OwnerAccess<'_>| {
            let copied = owner;
            assert!(owner == copied);
        })
        .expect("child owner should initialize");

    let root = runtime.owner().expect("root should start");
    root.with_access(|owner: OwnerAccess<'_>| {
        let copied = owner;
        assert!(owner == copied);
    });
    root.close().expect("root disposal should succeed");
}

#[test]
fn high_level_try_run_reports_an_active_root() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("first root should be created");

    assert!(matches!(
        runtime.owner(),
        Err(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::RuntimeAlreadyRunning
        )))
    ));

    root.close().expect("root disposal should succeed");
}
