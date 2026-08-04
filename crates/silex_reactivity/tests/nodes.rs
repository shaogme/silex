use silex_reactivity::{
    Callback, Derived, Effect, Memo, NodeRef, ReactiveError, ReadSignal, Runtime, StoredValue,
    WriteSignal,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct ReenterOnDrop<'scope> {
    setter: WriteSignal<'scope, i32>,
    called: Rc<Cell<bool>>,
    error: Rc<Cell<Option<ReactiveError>>>,
}

struct DropEvent {
    label: &'static str,
    events: Rc<RefCell<Vec<&'static str>>>,
}

impl Drop for DropEvent {
    fn drop(&mut self) {
        self.events.borrow_mut().push(self.label);
    }
}

impl Drop for ReenterOnDrop<'_> {
    fn drop(&mut self) {
        self.called.set(true);
        self.error.set(self.setter.try_set(1).err());
    }
}

#[test]
fn all_public_node_capabilities_are_copy() {
    fn assert_copy<T: Copy>(_: T) {}

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let signal = scope.rw_signal(0i32);
        let read = signal.read();
        let write = signal.write();
        let memo = scope.memo(move |_| read.get());
        let derived = scope.derived(move || 1i32);
        let effect = scope.effect(|| {});
        let stored = scope.stored(1i32);
        let callback = scope.callback(|_: ()| {});
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

        let _: Option<ReadSignal<'_, i32>> = Some(read);
        let _: Option<WriteSignal<'_, i32>> = Some(write);
        let _: Option<Memo<'_, i32>> = Some(memo);
        let _: Option<Derived<'_, i32>> = Some(derived);
        let _: Option<Effect<'_>> = Some(effect);
        let _: Option<StoredValue<'_, i32>> = Some(stored);
        let _: Option<Callback<'_, ()>> = Some(callback);
        let _: Option<NodeRef<'_, i32>> = Some(node_ref);
    });
}

#[test]
fn stored_callback_and_node_ref_are_scope_owned() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let stored = scope.stored(String::from("before"));
        stored.update(|value| value.push_str(" after"));
        assert!(stored.with(|value| value == "before after"));

        let called = Rc::new(Cell::new(0));
        let called_in_callback = called.clone();
        let callback = scope.callback(move |_: ()| {
            called_in_callback.set(called_in_callback.get() + 1);
        });
        callback.invoke(()).expect("callback should be alive");
        assert_eq!(called.get(), 1);

        let reference = scope.node_ref::<u32>();
        assert_eq!(reference.get(), None);
        reference.set(7).expect("node ref should be writable");
        assert_eq!(reference.get(), Some(7));
        reference.clear().expect("node ref should be clearable");
        assert_eq!(reference.get(), None);
    });
}

#[test]
fn callback_panic_restores_callback_for_the_next_invoke() {
    let mut runtime = Runtime::new();
    let called = Rc::new(Cell::new(0));
    let should_panic = Rc::new(Cell::new(true));

    runtime.child(|scope| {
        let called_in_callback = called.clone();
        let panic_in_callback = should_panic.clone();
        let callback = scope.callback(move |_: ()| {
            if panic_in_callback.replace(false) {
                panic!("callback panic");
            }
            called_in_callback.set(called_in_callback.get() + 1);
        });

        let panic = catch_unwind(AssertUnwindSafe(|| {
            callback.invoke(()).expect("callback exists");
        }));
        assert!(panic.is_err());
        callback.invoke(()).expect("callback should be restored");
    });

    assert_eq!(called.get(), 1);
}

#[test]
fn stored_update_panic_restores_the_stored_value() {
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
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
    runtime.child(|scope| {
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

#[test]
fn updating_another_signal_during_read_defers_effect_flush() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, _set_source) = scope.signal(0i32);
        let (other, set_other) = scope.signal(0i32);
        let runs_in_effect = runs.clone();
        scope.effect(move || {
            let _ = source.get();
            let _ = other.get();
            runs_in_effect.set(runs_in_effect.get() + 1);
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            source.with(|_| set_other.set(1));
        }));

        assert!(result.is_ok());
        assert_eq!(runs.get(), 2);
    });
}

#[test]
fn computation_payload_drop_can_reenter_after_state_borrow_is_released() {
    let mut runtime = Runtime::new();
    let called = Rc::new(Cell::new(false));
    let error = Rc::new(Cell::new(None));

    runtime.child(|scope| {
        let scope_copy = scope;
        let called_in_outer = called.clone();
        let error_in_outer = error.clone();
        scope.effect(move || {
            let (_source, set_source) = scope_copy.signal(0i32);
            let guard = ReenterOnDrop {
                setter: set_source,
                called: called_in_outer.clone(),
                error: error_in_outer.clone(),
            };
            scope_copy.effect(move || {
                let _ = &guard;
            });
        });
    });
    assert!(called.get());
    assert_eq!(error.get(), None);
}

#[test]
fn child_payloads_drop_before_parent_computation_payload() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = Runtime::new();

    runtime.child(|scope| {
        let scope_copy = scope;
        let parent_event = DropEvent {
            label: "parent",
            events: events.clone(),
        };
        let child_events = events.clone();
        scope.effect(move || {
            let _ = &parent_event;
            let signal_event = DropEvent {
                label: "signal",
                events: child_events.clone(),
            };
            let _ = scope_copy.signal(signal_event);

            let stored_event = DropEvent {
                label: "stored",
                events: child_events.clone(),
            };
            let _ = scope_copy.stored(stored_event);

            let callback_event = DropEvent {
                label: "callback",
                events: child_events.clone(),
            };
            let _ = scope_copy.callback(move |_: ()| {
                let _ = &callback_event;
            });

            let node_ref = scope_copy.node_ref::<DropEvent>();
            node_ref
                .set(DropEvent {
                    label: "node_ref",
                    events: child_events.clone(),
                })
                .expect("node ref type should match");
        });
    });
    let events = events.borrow();
    assert_eq!(events.len(), 5);
    let parent_position = events
        .iter()
        .position(|label| *label == "parent")
        .expect("parent payload should drop");
    for label in ["signal", "stored", "callback", "node_ref"] {
        let position = events
            .iter()
            .position(|event| *event == label)
            .expect("child payload should drop");
        assert!(position < parent_position, "{label} dropped after parent");
    }
}

#[test]
fn child_callback_payload_drop_can_schedule_an_active_parent_effect() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let called = Rc::new(Cell::new(false));
    let error = Rc::new(Cell::new(None));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let seen_in_effect = seen.clone();
        scope.effect(move || seen_in_effect.set(source.get()));

        let setter = set_source;
        scope.child(|child| {
            let drop_probe = ReenterOnDrop {
                setter,
                called: called.clone(),
                error: error.clone(),
            };
            let _callback = child.callback(move |_: ()| {
                let _ = &drop_probe;
            });
        });

        assert_eq!(seen.get(), 1);
    });

    assert!(called.get());
    assert_eq!(error.get(), None);
}

#[test]
fn stored_value_update_flushes_after_the_stored_payload_is_restored() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let stored = scope.stored(0i32);
        let seen_in_effect = seen.clone();
        scope.effect(move || seen_in_effect.set(source.get()));

        stored.update(|value| {
            *value = 1;
            set_source.set(1);
        });

        assert_eq!(seen.get(), 1);
    });
}
