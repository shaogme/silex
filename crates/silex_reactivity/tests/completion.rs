use silex_reactivity::{
    CallbackInvokeError, CompletionOnce, CompletionSender, ReactiveError, Runtime, unwind_safe,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct DropProbe(Rc<Cell<usize>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn completion_sender_submits_while_scope_is_active() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let received = Rc::new(Cell::new(0));
            let received_by_callback = received.clone();
            let sender = scope
                .completion_sender(unwind_safe(move |value: i32| {
                    received_by_callback.set(value);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");

            assert!(sender.submit(7).expect("completion submit"));
            assert!(sender.submit(9).expect("completion submit"));
            assert_eq!(received.get(), 9);
        })
        .expect("runtime child should succeed");
}

#[test]
fn explicit_completion_cancel_is_idempotent_and_invalidates_the_endpoint() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let called = Rc::new(Cell::new(false));
            let called_in_callback = called.clone();
            let sender = scope
                .completion_sender(unwind_safe(move |_: i32| {
                    called_in_callback.set(true);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");

            sender.cancel().expect("explicit completion cancel");
            sender.cancel().expect("repeated completion cancel");
            assert!(!sender.submit(1).expect("stale completion submit"));
            assert!(!called.get());
        })
        .expect("runtime child should succeed");
}

#[test]
fn unwind_safe_adapts_interior_mutable_repeating_callback() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let seen_in_callback = seen.clone();
            let sender = scope
                .completion_sender(unwind_safe(move |value: i32| {
                    seen_in_callback.borrow_mut().push(value);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");

            assert!(sender.submit(3).expect("completion submit"));
            assert!(sender.submit(5).expect("completion submit"));
        })
        .expect("runtime child should succeed");

    assert_eq!(&*seen.borrow(), &[3, 5]);
}

#[test]
fn completion_once_submits_once_and_reclaims_callback() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let probe = DropProbe(dropped.clone());
            let destination = scope
                .completion_once(unwind_safe(move |_: i32| {
                    std::hint::black_box(&probe);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            assert!(destination.submit(1).expect("completion submit"));
            assert!(!destination.submit(2).expect("stale completion submit"));
            assert_eq!(dropped.get(), 1);
        })
        .expect("runtime child should succeed");

    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let dropped_in_callback = dropped.clone();
            let destination = scope
                .completion_once(unwind_safe(move |_: i32| {
                    std::hint::black_box(&dropped_in_callback);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            assert!(destination.submit(3).expect("completion submit"));
        })
        .expect("runtime child should succeed");
    assert_eq!(dropped.get(), 1);
}

#[test]
fn completion_once_drop_cancels_callback_without_invoking_it() {
    let called = Rc::new(Cell::new(false));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let called_in_callback = called.clone();
            let destination = scope
                .completion_once(unwind_safe(move |_: i32| {
                    called_in_callback.set(true);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            drop(destination);
        })
        .expect("runtime child should succeed");
    assert!(!called.get());
}

#[test]
fn completion_sender_last_clone_drop_reclaims_callback() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let probe = DropProbe(dropped.clone());
            let sender = scope
                .completion_sender(unwind_safe(move |_: i32| {
                    std::hint::black_box(&probe);
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            let clone = sender.clone();
            drop(clone);
            assert_eq!(dropped.get(), 0);
            drop(sender);
            assert_eq!(dropped.get(), 1);
        })
        .expect("runtime child should succeed");
}

#[test]
fn completion_once_panic_still_reclaims_callback() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let probe = DropProbe(dropped.clone());
            let destination = scope
                .completion_once(unwind_safe(move |_: i32| {
                    std::hint::black_box(&probe);
                    panic!("completion panic");
                    #[allow(unreachable_code)]
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            let result = catch_unwind(AssertUnwindSafe(|| destination.submit(1)));
            assert!(result.is_err());
            assert!(!destination.submit(2).expect("stale completion submit"));
            assert_eq!(dropped.get(), 1);
        })
        .expect("runtime child should succeed");
}

#[test]
fn completion_sender_panic_still_reclaims_callback_and_closes() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let probe = DropProbe(dropped.clone());
            let destination = scope
                .completion_sender(unwind_safe(move |_: i32| {
                    std::hint::black_box(&probe);
                    panic!("completion sender panic");
                    #[allow(unreachable_code)]
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            let result = catch_unwind(AssertUnwindSafe(|| destination.submit(1)));

            assert!(result.is_err());
            assert!(!destination.submit(2).expect("stale completion submit"));
            assert_eq!(dropped.get(), 1);
        })
        .expect("runtime child should succeed");
}

#[test]
fn completion_once_is_invalid_after_child_scope_dispose() {
    let mut runtime = Runtime::new();
    let token = runtime
        .with_transient(|scope| {
            scope
                .with_transient(|child| {
                    child
                        .completion_once(unwind_safe(|_: i32| Ok::<(), ()>(())))
                        .expect("completion registration")
                })
                .expect("child scope execution")
        })
        .expect("runtime child");

    assert!(!token.submit(1).expect("stale completion submit"));
}

#[test]
fn completion_once_is_invalid_after_run_returns() {
    let mut runtime = Runtime::new();
    let token: CompletionOnce<i32, ()> = runtime
        .with_transient(|scope| {
            scope
                .completion_once(unwind_safe(|_: i32| Ok::<(), ()>(())))
                .expect("completion registration")
        })
        .expect("runtime child");

    assert!(!token.submit(1).expect("stale completion submit"));
}

#[test]
fn stale_completion_cannot_dispose_a_reused_scope_id() {
    let mut runtime = Runtime::new();
    let root = runtime.owner().expect("runtime root creation");
    let first_owner = root
        .access()
        .create_child()
        .expect("fallible reactive creation");
    let stale = first_owner
        .access()
        .completion_once(unwind_safe(|_: i32| Ok::<(), ()>(())))
        .expect("completion registration");
    first_owner.close().expect("owner disposal");

    let called = Rc::new(Cell::new(false));
    let called_in_callback = called.clone();
    let second_owner = root
        .access()
        .create_child()
        .expect("fallible reactive creation");
    let current = second_owner
        .access()
        .completion_once(unwind_safe(move |_: i32| {
            called_in_callback.set(true);
            Ok::<(), ()>(())
        }))
        .expect("completion registration");

    drop(stale);
    assert!(current.submit(1).expect("completion submit"));
    assert!(called.get());

    second_owner.close().expect("owner disposal");
    drop(current);
    drop(second_owner);
    drop(first_owner);
    root.close().expect("root cleanup");
}

#[test]
fn repeating_completion_returns_user_error_and_remains_active() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let seen = Rc::new(Cell::new(0));
            let seen_for_callback = seen.clone();
            let sender = scope
                .completion_sender(unwind_safe(move |value: i32| {
                    seen_for_callback.set(value);
                    if value == 1 {
                        Err("invalid value")
                    } else {
                        Ok(())
                    }
                }))
                .expect("completion registration");

            assert!(matches!(
                sender.submit(1),
                Err(CallbackInvokeError::User("invalid value"))
            ));
            assert_eq!(seen.get(), 1);
            assert!(sender.submit(2).expect("completion retry"));
            assert_eq!(seen.get(), 2);
        })
        .expect("runtime child should succeed");
}

#[test]
fn repeating_completion_does_not_roll_back_callback_side_effects_on_error() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let (signal, set_signal) = scope.signal(0).expect("fallible reactive creation");
            let sender = scope
                .completion_sender(unwind_safe(move |value: i32| {
                    set_signal.set(value).expect("signal update");
                    Err::<(), &'static str>("rejected")
                }))
                .expect("completion registration");

            assert!(matches!(
                sender.submit(7),
                Err(CallbackInvokeError::User("rejected"))
            ));
            assert_eq!(signal.get(), Ok(7));
        })
        .expect("runtime child should succeed");
}

#[test]
fn repeating_completion_reports_borrow_conflict_and_remains_active() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let nested = Rc::new(RefCell::new(None::<CompletionSender<i32, ()>>));
            let nested_for_callback = nested.clone();
            let sender = scope
                .completion_sender(unwind_safe(move |value: i32| {
                    if let Some(sender) = nested_for_callback.borrow().as_ref().cloned() {
                        assert!(matches!(
                            sender.submit(value),
                            Err(CallbackInvokeError::Runtime(ReactiveError::BorrowConflict))
                        ));
                    }
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            *nested.borrow_mut() = Some(sender.clone());

            assert!(sender.submit(1).expect("outer completion submit"));
            assert!(sender.submit(2).expect("completion remains active"));
        })
        .expect("test operation should succeed");
}

#[test]
fn completion_callback_can_cancel_itself_without_borrow_panic() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let destination = Rc::new(RefCell::new(None::<CompletionSender<i32, ()>>));
            let destination_in_callback = destination.clone();
            let sender = scope
                .completion_sender(unwind_safe(move |_: i32| {
                    let sender = destination_in_callback
                        .borrow()
                        .as_ref()
                        .cloned()
                        .expect("completion sender should be available");
                    let _ = sender.cancel();
                    Ok::<(), ()>(())
                }))
                .expect("completion registration");
            *destination.borrow_mut() = Some(sender.clone());

            assert!(sender.submit(1).expect("completion submit"));
            assert!(!sender.submit(2).expect("closed completion submit"));
            destination.borrow_mut().take();
        })
        .expect("test operation should succeed");
}

#[test]
fn once_completion_returns_user_error_and_closes() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let destination = scope
                .completion_once(unwind_safe(|_: i32| {
                    Err::<(), &'static str>("one shot failure")
                }))
                .expect("completion registration");

            assert!(matches!(
                destination.submit(1),
                Err(CallbackInvokeError::User("one shot failure"))
            ));
            assert!(
                !destination
                    .submit(2)
                    .expect("one-shot destination should be closed")
            );
        })
        .expect("runtime child should succeed");
}

#[test]
fn completion_error_can_borrow_scope_local_data() {
    let mut runtime = Runtime::new();
    runtime
        .with_transient(|scope| {
            let message = String::from("scope-local error");
            let expected = message.as_str();
            let sender = scope
                .completion_sender(unwind_safe(move |_: i32| Err::<(), &str>(expected)))
                .expect("completion registration");

            match sender.submit(1) {
                Err(CallbackInvokeError::User(error)) => assert_eq!(error, expected),
                other => panic!("unexpected completion result: {other:?}"),
            }
        })
        .expect("runtime child should succeed");
}
