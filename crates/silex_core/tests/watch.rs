use silex_core::{Runtime, WatchOptions};
use std::{cell::RefCell, rc::Rc};

#[test]
fn source_watch_uses_promotion_and_typed_callback_values() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(1_i32);
        let calls_in_callback = calls.clone();
        scope.watch(source, move |new, old| {
            calls_in_callback.borrow_mut().push((*new, old.copied()))
        });

        set_source.set(1);
        set_source.set(2);
        assert_eq!(calls.borrow().as_slice(), &[(2, Some(1))]);
    });
}

#[test]
fn getter_watch_supports_immediate_once_and_explicit_stop() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime.child(|scope| {
        let (source, set_source) = scope.signal(3_i32);
        let calls_in_callback = calls.clone();
        let watcher = scope.watch_getter_with_options(
            move || source.get(),
            move |new, old| calls_in_callback.borrow_mut().push((*new, old.copied())),
            WatchOptions::default().immediate().once(),
        );

        assert_eq!(calls.borrow().as_slice(), &[(3, None)]);
        assert!(!watcher.try_stop().expect("watcher should be stoppable"));
        set_source.set(4);
        assert_eq!(calls.borrow().as_slice(), &[(3, None)]);
    });
}

#[test]
fn tuple_source_watch_tracks_promoted_values_inside_a_batch() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime.child(|scope| {
        let (first, set_first) = scope.signal(1_i32);
        let (second, set_second) = scope.signal(2_i32);
        let calls_in_callback = calls.clone();
        scope.watch((first, second), move |new, old| {
            calls_in_callback.borrow_mut().push((*new, old.copied()))
        });

        scope.batch(|| {
            set_first.set(3);
            set_second.set(4);
        });
        assert_eq!(calls.borrow().as_slice(), &[((3, 4), Some((1, 2)))]);
    });
}
