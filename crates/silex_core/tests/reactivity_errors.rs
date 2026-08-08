use silex_core::traits::RxBase;
use silex_core::{ErrorHandler, ReactiveError, Runtime, Scope, SilexError};
use std::{cell::Cell, rc::Rc};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope.error_handler(|_| {})
}

#[test]
fn core_try_operations_preserve_borrow_conflicts() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (read, write) = scope.signal(1_i32);

        let read_then_write = read.try_with(|_| write.try_set(2));
        assert_eq!(read_then_write, Ok(Err(ReactiveError::BorrowConflict)),);

        let write_then_read = write.try_update(|_| read.try_get());
        assert_eq!(write_then_read, Ok(Err(ReactiveError::BorrowConflict)),);

        let write_then_write = write.try_update(|_| write.try_set(2));
        assert_eq!(write_then_write, Ok(Err(ReactiveError::BorrowConflict)),);

        assert_eq!(read.try_get(), Ok(1));
    });
}

#[test]
fn core_try_operations_preserve_stored_and_rx_errors() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let stored = scope.stored(1_i32);
        let stored_conflict = stored.try_with(|_| stored.try_update(|_| ()));
        assert_eq!(stored_conflict, Ok(Err(ReactiveError::BorrowConflict)),);

        let (read, write) = scope.signal(1_i32);
        let rx = read.into_rx();
        let rx_conflict = rx.try_with(|_| write.try_set(2));
        assert_eq!(rx_conflict, Ok(Err(ReactiveError::BorrowConflict)));
    });
}

#[test]
fn node_ref_keeps_empty_value_separate_from_runtime_errors() {
    let mut runtime = Runtime::new();
    let stale_error = Rc::new(Cell::new(None));
    let stale_error_for_cleanup = stale_error.clone();

    runtime.child(|scope| {
        let node_ref = scope.node_ref::<String>();
        assert_eq!(node_ref.try_get(), Ok(None));

        node_ref.try_load(String::from("value")).unwrap();
        assert_eq!(node_ref.try_get(), Ok(Some(String::from("value"))));

        node_ref.try_clear().unwrap();
        assert_eq!(node_ref.try_get(), Ok(None));

        let node_ref_for_cleanup = node_ref;
        scope
            .on_cleanup(
                move || {
                    stale_error_for_cleanup.set(node_ref_for_cleanup.try_get().err());
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    assert_eq!(stale_error.get(), Some(ReactiveError::NoSuchNode));
}

#[test]
fn stale_core_trait_access_returns_no_such_node() {
    let mut runtime = Runtime::new();
    let stale_error = Rc::new(Cell::new(None));
    let stale_error_for_cleanup = stale_error.clone();

    runtime.child(|scope| {
        let (read, write) = scope.signal(1_i32);
        scope
            .on_cleanup(
                move || {
                    assert_eq!(read.try_get(), Err(ReactiveError::NoSuchNode));
                    assert_eq!(read.try_track(), Err(ReactiveError::NoSuchNode));
                    assert_eq!(write.try_set(2), Err(ReactiveError::NoSuchNode));
                    assert_eq!(write.try_notify(), Err(ReactiveError::NoSuchNode));
                    stale_error_for_cleanup.set(read.try_get().err());
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    assert_eq!(stale_error.get(), Some(ReactiveError::NoSuchNode));
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
    runtime.child(|scope| {
        assert!(scope.on_cleanup(|| Ok(()), handler(scope)).is_ok());
        let owner = scope.try_owned_scope().expect("owner is active");
        assert!(owner.on_cleanup(|| Ok(()), handler(scope)).is_ok());
        owner.dispose();

        assert!(matches!(
            owner.on_cleanup(|| Ok(()), handler(scope)),
            Err(SilexError::Reactivity(ReactiveError::NoSuchNode))
        ));
        assert!(matches!(
            owner.try_child(),
            Err(SilexError::Reactivity(ReactiveError::NoSuchNode))
        ));
    });
}
