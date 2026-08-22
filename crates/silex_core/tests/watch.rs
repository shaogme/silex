use silex_core::{
    EffectPhase, ErrorHandlerToken, OwnerAccess, Runtime, WatchOptions, traits::RxGet,
};
use std::{cell::RefCell, rc::Rc};

fn handler<'owner>(owner: OwnerAccess<'owner>) -> ErrorHandlerToken<'owner> {
    owner
        .error_handler(|_| {})
        .expect("error handler should register")
}

#[test]
fn source_watch_uses_promotion_and_typed_callback_values() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .with_transient(|owner| {
            let (source, set_source) = owner.signal(1_i32).expect("signal should initialize");
            let calls_in_callback = calls.clone();
            owner
                .watch(
                    EffectPhase::Normal,
                    source,
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        Ok(())
                    },
                    handler(owner),
                )
                .expect("watch should register");

            set_source.set(1).expect("signal should be writable");
            set_source.set(2).expect("signal should be writable");
            assert_eq!(calls.borrow().as_slice(), &[(2, Some(1))]);
        })
        .expect("child owner should initialize");
}

#[test]
fn getter_watch_supports_immediate_once_and_explicit_stop() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .with_transient(|owner| {
            let (source, set_source) = owner.signal(3_i32).expect("signal should initialize");
            let calls_in_callback = calls.clone();
            let watcher = owner
                .watch_getter_with_options(
                    EffectPhase::Normal,
                    move || source.get(),
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        Ok(())
                    },
                    handler(owner),
                    WatchOptions::default().immediate().once(),
                )
                .expect("watcher should register");

            assert_eq!(calls.borrow().as_slice(), &[(3, None)]);
            assert!(!watcher.stop().expect("watcher should be stoppable"));
            set_source.set(4).expect("signal should be writable");
            assert_eq!(calls.borrow().as_slice(), &[(3, None)]);
        })
        .expect("child owner should initialize");
}

#[test]
fn tuple_source_watch_tracks_promoted_values_inside_a_batch() {
    let mut runtime = Runtime::new();
    let calls = Rc::new(RefCell::new(Vec::new()));

    runtime
        .with_transient(|owner| {
            let (first, set_first) = owner.signal(1_i32).expect("signal should initialize");
            let (second, set_second) = owner.signal(2_i32).expect("signal should initialize");
            let calls_in_callback = calls.clone();
            owner
                .watch(
                    EffectPhase::Normal,
                    (first, second),
                    move |new, old| {
                        calls_in_callback.borrow_mut().push((*new, old.copied()));
                        Ok(())
                    },
                    handler(owner),
                )
                .expect("watch should register");

            owner
                .batch(|| {
                    set_first.set(3).expect("signal should be writable");
                    set_second.set(4).expect("signal should be writable");
                })
                .expect("batch should flush");
            assert_eq!(calls.borrow().as_slice(), &[((3, 4), Some((1, 2)))]);
        })
        .expect("child owner should initialize");
}
