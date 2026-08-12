use silex_core::{
    ErrorHandler, Memo, ReactiveError, ReactiveInput, ReadSignal, Runtime, RwSignal, Rx, Scope,
    Signal, SilexError, StoredValue,
};
use std::{cell::RefCell, rc::Rc};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn supported_values_materialize_each_wrapper() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let signal: Signal<'_, String> = "signal"
                .into_reactive_input(scope)
                .expect("signal should initialize");
            let read: ReadSignal<'_, i32> = 7
                .into_reactive_input(scope)
                .expect("read signal should initialize");
            let rw: RwSignal<'_, bool> = true
                .into_reactive_input(scope)
                .expect("rw signal should initialize");
            let memo: Memo<'_, f64> = 1.5
                .into_reactive_input(scope)
                .expect("memo should initialize");
            let stored: StoredValue<'_, char> = 'x'
                .into_reactive_input(scope)
                .expect("stored value should initialize");
            let rx: Rx<'_, usize> = 3usize
                .into_reactive_input(scope)
                .expect("rx should initialize");

            assert_eq!(signal.get().expect("signal should be readable"), "signal");
            assert_eq!(read.get().expect("read signal should be readable"), 7);
            assert!(rw.get().expect("rw signal should be readable"));
            assert_eq!(memo.get().expect("memo should be readable"), 1.5);
            assert_eq!(
                stored
                    .with(|value| *value)
                    .expect("stored should be readable"),
                'x'
            );
            assert_eq!(rx.get().expect("rx should be readable"), 3);
        })
        .expect("child scope should initialize");
}

#[test]
fn borrowed_str_is_owned_by_the_target_node() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let value = String::from("borrowed");
            let signal: Signal<'_, String> = value
                .as_str()
                .into_reactive_input(scope)
                .expect("borrowed string should initialize");
            drop(value);

            assert_eq!(signal.get().expect("signal should be readable"), "borrowed");
        })
        .expect("child scope should initialize");
}

#[test]
fn existing_sources_keep_identity_and_runtime_inputs() {
    let mut runtime = Runtime::new();
    runtime
        .child(|scope| {
            let (read, _) = scope.signal(1_i32).expect("signal should initialize");
            let rw = scope.rw_signal(2_i32).expect("rw signal should initialize");
            let memo = scope.memo(|_| 3_i32).expect("memo should initialize");
            let stored = scope.stored(4_i32).expect("stored value should initialize");
            let rx = scope.constant(5_i32).expect("constant should initialize");

            let read_inputs = read.into_rx().runtime_inputs();
            let signal_from_read: Signal<'_, i32> = read
                .into_reactive_input(scope)
                .expect("signal conversion should succeed");
            assert_eq!(signal_from_read.into_rx().runtime_inputs(), read_inputs);
            assert_eq!(signal_from_read, read.into());

            let read_from_rw: ReadSignal<'_, i32> = rw
                .into_reactive_input(scope)
                .expect("read conversion should succeed");
            assert_eq!(read_from_rw, rw.read_signal());

            let rw_from_rw: RwSignal<'_, i32> = rw
                .into_reactive_input(scope)
                .expect("rw conversion should succeed");
            assert_eq!(rw_from_rw, rw);

            let memo_from_memo: Memo<'_, i32> = memo
                .into_reactive_input(scope)
                .expect("memo conversion should succeed");
            assert_eq!(memo_from_memo, memo);

            let stored_from_stored: StoredValue<'_, i32> = stored
                .into_reactive_input(scope)
                .expect("stored conversion should succeed");
            assert_eq!(stored_from_stored, stored);

            let signal_from_rx: Signal<'_, i32> = rx
                .into_reactive_input(scope)
                .expect("signal conversion should succeed");
            assert_eq!(
                signal_from_rx.into_rx().runtime_inputs(),
                rx.runtime_inputs()
            );
            let rx_from_signal: Rx<'_, i32> = signal_from_rx
                .into_reactive_input(scope)
                .expect("rx conversion should succeed");
            assert_eq!(rx_from_signal.runtime_inputs(), rx.runtime_inputs());
        })
        .expect("child scope should initialize");
}

#[test]
fn foreign_sources_are_not_materialized_as_local_constants() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();

    let result = first
        .child(|first_scope| {
            let source: Signal<'_, i32> = 1
                .into_reactive_input(first_scope)
                .expect("source should initialize");
            let converted: Signal<'_, i32> = source
                .into_reactive_input(first_scope)
                .expect("conversion should succeed");
            let inputs = converted.into_rx().runtime_inputs();

            second
                .child(|second_scope| second_scope.validate_inputs(&inputs))
                .and_then(|result| result)
        })
        .expect("child scope should initialize");

    assert!(matches!(
        result,
        Err(SilexError::Reactivity(ReactiveError::RuntimeMismatch))
    ));
}

#[test]
fn signal_and_stored_value_keep_source_specific_cleanup_access() {
    let mut runtime = Runtime::new();
    let observed = Rc::new(RefCell::new((None, None)));

    runtime
        .child(|scope| {
            let (read, _) = scope.signal(7_i32).expect("signal should initialize");
            let signal_from_read: Signal<'_, i32> = read.into();
            let signal_from_stored: Signal<'_, i32> = 9
                .into_reactive_input(scope)
                .expect("stored-backed signal should initialize");
            let observed_for_cleanup = observed.clone();
            scope
                .on_cleanup(
                    move || {
                        let signal_error = signal_from_read
                            .get()
                            .expect_err("raw signal should be inactive during cleanup");
                        let stored_value = signal_from_stored
                            .get()
                            .expect("stored-backed signal should remain available during cleanup");
                        let mut observed = observed_for_cleanup.borrow_mut();
                        observed.0 = Some(signal_error);
                        observed.1 = Some(stored_value);
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("cleanup should register");
        })
        .expect("child scope should initialize");

    let observed = observed.borrow();
    assert!(matches!(
        observed.0,
        Some(SilexError::Reactivity(ReactiveError::NoSuchNode))
    ));
    assert_eq!(observed.1, Some(9));
}
