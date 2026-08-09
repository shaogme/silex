use silex_core::{
    ErrorHandler, Memo, ReactiveError, ReactiveInput, ReadSignal, Runtime, RwSignal, Rx, Scope,
    Signal, SilexError, StoredValue,
};
use std::{cell::Cell, rc::Rc};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandler<'scope, SilexError> {
    scope.error_handler(|_| {})
}

#[test]
fn supported_values_materialize_each_wrapper() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let signal: Signal<'_, String> = "signal".into_reactive_input(scope);
        let read: ReadSignal<'_, i32> = 7.into_reactive_input(scope);
        let rw: RwSignal<'_, bool> = true.into_reactive_input(scope);
        let memo: Memo<'_, f64> = 1.5.into_reactive_input(scope);
        let stored: StoredValue<'_, char> = 'x'.into_reactive_input(scope);
        let rx: Rx<'_, usize> = 3usize.into_reactive_input(scope);

        assert_eq!(signal.get(), "signal");
        assert_eq!(read.get(), 7);
        assert!(rw.get());
        assert_eq!(memo.get(), 1.5);
        assert_eq!(stored.with(|value| *value), 'x');
        assert_eq!(rx.get(), 3);
    });
}

#[test]
fn borrowed_str_is_owned_by_the_target_node() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let value = String::from("borrowed");
        let signal: Signal<'_, String> = value.as_str().into_reactive_input(scope);
        drop(value);

        assert_eq!(signal.get(), "borrowed");
    });
}

#[test]
fn existing_sources_keep_identity_and_runtime_inputs() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let (read, _) = scope.signal(1_i32);
        let rw = scope.rw_signal(2_i32);
        let memo = scope.memo(|_| 3_i32);
        let stored = scope.stored(4_i32);
        let rx = scope.constant(5_i32);

        let read_inputs = read.into_rx().runtime_inputs();
        let signal_from_read: Signal<'_, i32> = read.into_reactive_input(scope);
        assert_eq!(signal_from_read.into_rx().runtime_inputs(), read_inputs);
        assert_eq!(signal_from_read, read.into());

        let read_from_rw: ReadSignal<'_, i32> = rw.into_reactive_input(scope);
        assert_eq!(read_from_rw, rw.read_signal());

        let rw_from_rw: RwSignal<'_, i32> = rw.into_reactive_input(scope);
        assert_eq!(rw_from_rw, rw);

        let memo_from_memo: Memo<'_, i32> = memo.into_reactive_input(scope);
        assert_eq!(memo_from_memo, memo);

        let stored_from_stored: StoredValue<'_, i32> = stored.into_reactive_input(scope);
        assert_eq!(stored_from_stored, stored);

        let signal_from_rx: Signal<'_, i32> = rx.into_reactive_input(scope);
        assert_eq!(
            signal_from_rx.into_rx().runtime_inputs(),
            rx.runtime_inputs()
        );
        let rx_from_signal: Rx<'_, i32> = signal_from_rx.into_reactive_input(scope);
        assert_eq!(rx_from_signal.runtime_inputs(), rx.runtime_inputs());
    });
}

#[test]
fn foreign_sources_are_not_materialized_as_local_constants() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();

    let result = first.child(|first_scope| {
        let source: Signal<'_, i32> = 1.into_reactive_input(first_scope);
        let converted: Signal<'_, i32> = source.into_reactive_input(first_scope);
        let inputs = converted.into_rx().runtime_inputs();

        second.child(|second_scope| second_scope.try_validate_inputs(&inputs))
    });

    assert!(matches!(
        result,
        Err(SilexError::Reactivity(ReactiveError::RuntimeMismatch))
    ));
}

#[test]
fn constant_nodes_are_cleaned_up_with_their_scope() {
    let mut runtime = Runtime::new();
    let stale = Rc::new(Cell::new(false));
    let stale_for_cleanup = stale.clone();

    runtime.child(|scope| {
        let signal: Signal<'_, i32> = 9.into_reactive_input(scope);
        scope
            .on_cleanup(
                move || {
                    stale_for_cleanup.set(signal.try_get() == Err(ReactiveError::NoSuchNode));
                    Ok(())
                },
                handler(scope),
            )
            .expect("cleanup should register");
    });

    assert!(stale.get());
}
