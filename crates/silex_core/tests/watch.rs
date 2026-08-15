use silex_core::{ErrorHandlerToken, Runtime, Scope, WatchOptions};
use std::{cell::RefCell, rc::Rc};

fn handler<'scope>(scope: Scope<'scope>) -> ErrorHandlerToken<'scope> {
    scope
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn source_watch_uses_promotion_and_typed_callback_values() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(1_i32).expect("signal should initialize");
            let calls_in_callback = calls.clone();
            scope
                .watch(
                    source,
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("watch should register");

            set_source.set(1).expect("signal should be writable");
            set_source.set(2).expect("signal should be writable");
            assert_eq!(calls.borrow().as_slice(), &[(2, Some(1))]);
        })
        .expect("child scope should initialize");
}

#[test]
fn getter_watch_supports_immediate_once_and_explicit_stop() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .child(|scope| {
            let (source, set_source) = scope.signal(3_i32).expect("signal should initialize");
            let calls_in_callback = calls.clone();
            let watcher = scope
                .watch_getter_with_options(
                    move || source.get(),
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        Ok(())
                    },
                    handler(scope),
                    WatchOptions::default().immediate().once(),
                )
                .expect("watcher should register");

            assert_eq!(calls.borrow().as_slice(), &[(3, None)]);
            assert!(!watcher.stop().expect("watcher should be stoppable"));
            set_source.set(4).expect("signal should be writable");
            assert_eq!(calls.borrow().as_slice(), &[(3, None)]);
        })
        .expect("child scope should initialize");
}

#[test]
fn tuple_source_watch_tracks_promoted_values_inside_a_batch() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .child(|scope| {
            let (first, set_first) = scope.signal(1_i32).expect("signal should initialize");
            let (second, set_second) = scope.signal(2_i32).expect("signal should initialize");
            let calls_in_callback = calls.clone();
            scope
                .watch(
                    (first, second),
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        Ok(())
                    },
                    handler(scope),
                )
                .expect("watch should register");

            scope
                .batch(|| {
                    set_first.set(3).expect("signal should be writable");
                    set_second.set(4).expect("signal should be writable");
                })
                .expect("batch should flush");
            assert_eq!(calls.borrow().as_slice(), &[((3, 4), Some((1, 2)))]);
        })
        .expect("child scope should initialize");
}
