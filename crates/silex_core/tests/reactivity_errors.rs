use silex_core::{
    ErrorHandlerToken, OwnerAccess, ReactiveError, Runtime, SilexError, SilexErrorKind,
    traits::{RxGet, RxRead},
};
use std::{cell::Cell, rc::Rc};

fn handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn core_try_operations_preserve_borrow_conflicts() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let read = owner.signal(1_i32).expect("signal should initialize");

            let read_then_write = read.with(|_| read.set(2));
            assert!(matches!(
                read_then_write,
                Ok(Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                    ReactiveError::BorrowConflict
                ))))
            ));

            let write_then_read = read.write_signal().update(|_| read.get());
            assert!(matches!(
                write_then_read,
                Ok(Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                    ReactiveError::BorrowConflict
                ))))
            ));

            let write_then_write = read.write_signal().update(|_| read.set(2));
            assert!(matches!(
                write_then_write,
                Ok(Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                    ReactiveError::BorrowConflict
                ))))
            ));

            assert!(matches!(read.get(), Ok(1)));
        })
        .expect("child owner should initialize");
}

#[test]
fn core_try_operations_preserve_stored_and_rx_errors() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            let stored = owner.stored(1_i32).expect("stored value should initialize");
            let stored_conflict = stored.with(|_| stored.update(|_| ()));
            assert!(matches!(
                stored_conflict,
                Ok(Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                    ReactiveError::BorrowConflict
                ))))
            ));

            let read = owner.signal(1_i32).expect("signal should initialize");
            let rx = read.into_rx();
            let rx_conflict = rx.with(|_| read.set(2));
            assert!(matches!(
                rx_conflict,
                Ok(Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                    ReactiveError::BorrowConflict
                ))))
            ));
        })
        .expect("child owner should initialize");
}

#[test]
fn node_ref_keeps_empty_value_separate_from_runtime_errors() {
    let mut runtime = Runtime::new();
    let stale_error = Rc::new(Cell::new(None));
    let stale_error_for_cleanup = stale_error.clone();

    runtime
        .with_transient(|owner| {
            let node_ref = owner
                .node_ref::<String>()
                .expect("node ref should initialize");
            assert!(matches!(node_ref.get(), Ok(None)));

            node_ref.load(String::from("value")).unwrap();
            assert!(matches!(node_ref.get(), Ok(Some(value)) if value == "value"));

            node_ref.clear().unwrap();
            assert!(matches!(node_ref.get(), Ok(None)));

            let node_ref_for_cleanup = node_ref;
            owner
                .on_cleanup(
                    move || {
                        stale_error_for_cleanup.set(node_ref_for_cleanup.get().err());
                        Ok(())
                    },
                    handler(owner),
                )
                .expect("cleanup should register");
        })
        .expect("child owner should initialize");

    assert!(matches!(
        stale_error.take(),
        Some(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::NoSuchNode
        )))
    ));
}

#[test]
fn stale_core_read_returns_no_such_node_and_track_is_inactive() {
    let mut runtime = Runtime::new();
    let stale_error = Rc::new(Cell::new(None));
    let stale_error_for_cleanup = stale_error.clone();

    runtime
        .with_transient(|owner| {
            let read = owner.signal(1_i32).expect("signal should initialize");
            owner
                .on_cleanup(
                    move || {
                        assert!(matches!(
                            read.get(),
                            Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                                ReactiveError::NoSuchNode
                            )))
                        ));
                        assert!(matches!(
                            read.set(2),
                            Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                                ReactiveError::NoSuchNode
                            )))
                        ));
                        assert!(matches!(
                            read.write_signal().notify(),
                            Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                                ReactiveError::NoSuchNode
                            )))
                        ));
                        stale_error_for_cleanup.set(read.get().err());
                        Ok(())
                    },
                    handler(owner),
                )
                .expect("cleanup should register");
        })
        .expect("child owner should initialize");

    assert!(matches!(
        stale_error.take(),
        Some(SilexError::Fatal(SilexErrorKind::Reactivity(
            ReactiveError::NoSuchNode
        )))
    ));
}

#[test]
fn runtime_errors_are_matchable_through_silex_error() {
    let error = SilexErrorKind::from(ReactiveError::RuntimeMismatch);
    assert!(matches!(
        error,
        SilexErrorKind::Reactivity(ReactiveError::RuntimeMismatch)
    ));
}

#[test]
fn core_owner_registration_exposes_inactive_errors() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|owner| {
            assert!(owner.on_cleanup(|| Ok(()), handler(owner)).is_ok());
            let child = owner.create_child().expect("owner is active");
            let child_owner = child.access();
            let child_handler = handler(child_owner);
            assert!(child_owner.on_cleanup(|| Ok(()), &child_handler).is_ok());
            child.close().expect("child owner should close");

            assert!(matches!(
                child_owner.on_cleanup(|| Ok(()), &child_handler),
                Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                    ReactiveError::NoSuchNode
                )))
            ));
            assert!(matches!(
                child_owner.with_transient(|_| ()),
                Err(SilexError::Fatal(SilexErrorKind::Reactivity(
                    ReactiveError::NoSuchNode
                )))
            ));
        })
        .expect("child owner should initialize");
}
