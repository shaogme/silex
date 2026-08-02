use silex_reactivity::{
    Callback, Derived, Effect, Memo, NodeRef, ReadSignal, Runtime, StoredValue, WriteSignal,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn all_public_node_capabilities_are_copy() {
    fn assert_copy<T: Copy>(_: T) {}

    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let signal = scope.rw_signal(0i32);
        let read = signal.read();
        let write = signal.write();
        let memo = scope.memo(move |_| read.get());
        let derived = scope.derived(move || 1i32);
        let effect = scope.effect(|| {});
        let stored = scope.stored(1i32);
        let callback = scope.callback(|_| {});
        let node_ref = scope.node_ref::<i32>();

        assert_copy(read);
        assert_copy(write);
        assert_copy(signal);
        assert_copy(memo);
        assert_copy(derived);
        assert_copy(effect);
        assert_copy(stored);
        assert_copy(callback);
        assert_copy(node_ref);

        let _: Option<ReadSignal<'_, '_, i32>> = Some(read);
        let _: Option<WriteSignal<'_, '_, i32>> = Some(write);
        let _: Option<Memo<'_, '_, i32>> = Some(memo);
        let _: Option<Derived<'_, '_, i32>> = Some(derived);
        let _: Option<Effect<'_, '_>> = Some(effect);
        let _: Option<StoredValue<'_, '_, i32>> = Some(stored);
        let _: Option<Callback<'_, '_>> = Some(callback);
        let _: Option<NodeRef<'_, '_, i32>> = Some(node_ref);
    });
}

#[test]
fn stored_callback_and_node_ref_are_scope_owned() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let stored = scope.stored(String::from("before"));
        stored.update(|value| value.push_str(" after"));
        assert!(stored.with(|value| value == "before after"));

        let called = Rc::new(Cell::new(0));
        let called_in_callback = called.clone();
        let callback = scope.callback(move |_| {
            called_in_callback.set(called_in_callback.get() + 1);
        });
        callback
            .invoke(Box::new(()))
            .expect("callback should be alive");
        assert_eq!(called.get(), 1);

        let reference = scope.node_ref::<u32>();
        assert_eq!(reference.get(), None);
        reference.set(7).expect("node ref should be writable");
        assert_eq!(reference.get(), Some(7));
    });
}

#[test]
fn callback_panic_restores_callback_for_the_next_invoke() {
    let mut runtime = Runtime::new();
    let called = Rc::new(Cell::new(0));
    let should_panic = Rc::new(Cell::new(true));

    runtime.run(|scope| {
        let called_in_callback = called.clone();
        let panic_in_callback = should_panic.clone();
        let callback = scope.callback(move |_| {
            if panic_in_callback.replace(false) {
                panic!("callback panic");
            }
            called_in_callback.set(called_in_callback.get() + 1);
        });

        let panic = catch_unwind(AssertUnwindSafe(|| {
            callback.invoke(Box::new(())).expect("callback exists");
        }));
        assert!(panic.is_err());
        callback
            .invoke(Box::new(()))
            .expect("callback should be restored");
    });

    assert_eq!(called.get(), 1);
}

#[test]
fn stored_update_panic_restores_the_stored_value() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let stored = scope.stored(String::from("before"));
        let panic = catch_unwind(AssertUnwindSafe(|| {
            stored.update(|_| panic!("stored update panic"));
        }));
        assert!(panic.is_err());
        assert!(stored.with(|value| value == "before"));

        stored.update(|value| value.push_str(" after"));
        assert!(stored.with(|value| value == "before after"));
    });
}

#[test]
fn updating_one_signal_can_read_another_signal() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let (other, set_other) = scope.signal(2i32);
        set_source
            .try_update(|value| {
                *value += other.get();
            })
            .expect("updating one signal should release state borrow");
        assert_eq!(source.get(), 3);

        set_other.set(4);
        set_source.update(|value| *value += other.get());
        assert_eq!(source.get(), 7);
    });
}
