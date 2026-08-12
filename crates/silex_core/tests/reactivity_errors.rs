use silex_core::traits::RxBase;
use silex_core::{ErrorHandler, ReactiveError, Runtime, Scope, SilexError};
use std::{cell::Cell, rc::Rc};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn core_try_operations_preserve_borrow_conflicts() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, write) = scope.signal(1_i32).expect("signal should initialize");

            let read_then_write = read.with(|_| write.set(2));
            assert!(matches!(
                read_then_write,
                Ok(Err(ReactiveError::BorrowConflict))
            ));

            let write_then_read = write.update(|_| read.get());
            assert!(matches!(
                write_then_read,
                Ok(Err(SilexError::Reactivity(ReactiveError::BorrowConflict)))
            ));

            let write_then_write = write.update(|_| write.set(2));
            assert!(matches!(
                write_then_write,
                Ok(Err(ReactiveError::BorrowConflict))
            ));

            assert!(matches!(read.get(), Ok(1)));
        })
        .expect("child scope should initialize");
}

#[test]
fn core_try_operations_preserve_stored_and_rx_errors() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let stored = scope.stored(1_i32).expect("stored value should initialize");
            let stored_conflict = stored.with(|_| stored.update(|_| ()));
            assert!(matches!(
                stored_conflict,
                Ok(Err(ReactiveError::BorrowConflict))
            ));

            let (read, write) = scope.signal(1_i32).expect("signal should initialize");
            let rx = read.into_rx();
            let rx_conflict = rx.with(|_| write.set(2));
            assert!(matches!(
                rx_conflict,
                Ok(Err(ReactiveError::BorrowConflict))
            ));
        })
        .expect("child scope should initialize");
}

#[test]
fn node_ref_keeps_empty_value_separate_from_runtime_errors() {
    let mut runtime = Runtime::new();
    let stale_error = Rc::new(Cell::new(None));
    let stale_error_for_cleanup = stale_error.clone();

    runtime
        .child(|scope| {
            let node_ref = scope
                .node_ref::<String>()
                .expect("node ref should initialize");
            assert_eq!(node_ref.get(), Ok(None));

            node_ref.load(String::from("value")).unwrap();
            assert_eq!(node_ref.get(), Ok(Some(String::from("value"))));

            node_ref.clear().unwrap();
            assert_eq!(node_ref.get(), Ok(None));

            let node_ref_for_cleanup = node_ref;
            scope
                .on_cleanup(
                    move || {
                        stale_error_for_cleanup.set(node_ref_for_cleanup.get().err());
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("child scope should initialize");

    assert!(matches!(
        stale_error.take(),
        Some(ReactiveError::NoSuchNode)
    ));
}

#[test]
fn stale_core_trait_access_returns_no_such_node() {
    let mut runtime = Runtime::new();
    let stale_error = Rc::new(Cell::new(None));
    let stale_error_for_cleanup = stale_error.clone();

    runtime
        .child(|scope| {
            let (read, write) = scope.signal(1_i32).expect("signal should initialize");
            scope
                .on_cleanup(
                    move || {
                        assert!(matches!(
                            read.get(),
                            Err(SilexError::Reactivity(ReactiveError::NoSuchNode))
                        ));
                        assert!(matches!(
                            read.track(),
                            Err(SilexError::Reactivity(ReactiveError::NoSuchNode))
                        ));
                        assert!(matches!(write.set(2), Err(ReactiveError::NoSuchNode)));
                        assert!(matches!(write.notify(), Err(ReactiveError::NoSuchNode)));
                        stale_error_for_cleanup.set(read.get().err());
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("child scope should initialize");

    assert!(matches!(
        stale_error.take(),
        Some(SilexError::Reactivity(ReactiveError::NoSuchNode))
    ));
}

#[test]
fn runtime_errors_are_matchable_through_silex_error() {
    let error = SilexError::from(ReactiveError::RuntimeMismatch);
    assert!(matches!(
        error,
        SilexError::Reactivity(ReactiveError::RuntimeMismatch)
    ));
}

#[test]
fn core_owner_registration_exposes_inactive_errors() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            assert!(scope.on_cleanup(|| Ok(()), handler(scope)).is_ok());
            let owner = scope.owned_scope().expect("owner is active");
            assert!(owner.on_cleanup(|| Ok(()), handler(scope)).is_ok());
            owner.dispose().expect("owned scope should dispose");

            assert!(matches!(
                owner.on_cleanup(|| Ok(()), handler(scope)),
                Err(SilexError::Reactivity(ReactiveError::NoSuchNode))
            ));
            assert!(matches!(
                owner.child(),
                Err(SilexError::Reactivity(ReactiveError::NoSuchNode))
            ));
        })
        .expect("child scope should initialize");
}
