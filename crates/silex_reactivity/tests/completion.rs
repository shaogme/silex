use silex_reactivity::{CompletionOnce, Runtime, unwind_safe};
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
    runtime.child(|scope| {
        let received = Rc::new(Cell::new(0));
        let received_by_callback = received.clone();
        let sender = scope.completion_sender(unwind_safe(move |value: i32| {
            received_by_callback.set(value);
        }));

        assert!(sender.submit(7));
        assert!(sender.submit(9));
        assert_eq!(received.get(), 9);
    });
}

#[test]
fn unwind_safe_adapts_interior_mutable_repeating_callback() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let seen_in_callback = seen.clone();
        let sender = scope.completion_sender(unwind_safe(move |value: i32| {
            seen_in_callback.borrow_mut().push(value);
        }));

        assert!(sender.submit(3));
        assert!(sender.submit(5));
    });

    assert_eq!(&*seen.borrow(), &[3, 5]);
}

#[test]
fn completion_once_submits_once_and_reclaims_callback() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let probe = DropProbe(dropped.clone());
        let destination = scope.completion_once(unwind_safe(move |_: i32| {
            let _ = &probe;
        }));
        assert!(destination.submit(1));
        assert!(!destination.submit(2));
        assert_eq!(dropped.get(), 1);
    });

    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let dropped_in_callback = dropped.clone();
        let destination = scope.completion_once(unwind_safe(move |_: i32| {
            let _ = dropped_in_callback;
        }));
        assert!(destination.submit(3));
    });
    assert_eq!(dropped.get(), 1);
}

#[test]
fn completion_once_drop_cancels_callback_without_invoking_it() {
    let called = Rc::new(Cell::new(false));
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let called_in_callback = called.clone();
        let destination =
            scope.completion_once(unwind_safe(move |_: i32| called_in_callback.set(true)));
        drop(destination);
    });
    assert!(!called.get());
}

#[test]
fn completion_sender_last_clone_drop_reclaims_callback() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let probe = DropProbe(dropped.clone());
        let sender = scope.completion_sender(unwind_safe(move |_: i32| {
            let _ = &probe;
        }));
        let clone = sender.clone();
        drop(clone);
        assert_eq!(dropped.get(), 0);
        drop(sender);
        assert_eq!(dropped.get(), 1);
    });
}

#[test]
fn completion_once_panic_still_reclaims_callback() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let probe = DropProbe(dropped.clone());
        let destination = scope.completion_once(unwind_safe(move |_: i32| {
            let _ = &probe;
            panic!("completion panic");
        }));
        let result = catch_unwind(AssertUnwindSafe(|| destination.submit(1)));
        assert!(result.is_err());
        assert!(!destination.submit(2));
        assert_eq!(dropped.get(), 1);
    });
}

#[test]
fn completion_sender_panic_still_reclaims_callback_and_closes() {
    let dropped = Rc::new(Cell::new(0));
    let mut runtime = Runtime::new();
    runtime.child(|scope| {
        let probe = DropProbe(dropped.clone());
        let destination = scope.completion_sender(unwind_safe(move |_: i32| {
            let _ = &probe;
            panic!("completion sender panic");
        }));
        let result = catch_unwind(AssertUnwindSafe(|| destination.submit(1)));

        assert!(result.is_err());
        assert!(!destination.submit(2));
        assert_eq!(dropped.get(), 1);
    });
}

#[test]
fn completion_once_is_invalid_after_child_scope_dispose() {
    let mut runtime = Runtime::new();
    let token =
        runtime.child(|scope| scope.child(|child| child.completion_once(unwind_safe(|_: i32| {}))));

    assert!(!token.submit(1));
}

#[test]
fn completion_once_is_invalid_after_run_returns() {
    let mut runtime = Runtime::new();
    let token: CompletionOnce<i32> =
        runtime.child(|scope| scope.completion_once(unwind_safe(|_: i32| {})));

    assert!(!token.submit(1));
}

#[test]
fn stale_completion_cannot_dispose_a_reused_scope_id() {
    let mut runtime = Runtime::new();
    let root = runtime.run();
    let first_owner = root.scope().owned_scope();
    let stale = first_owner.completion_once(unwind_safe(|_: i32| {}));
    first_owner.dispose();

    let called = Rc::new(Cell::new(false));
    let called_in_callback = called.clone();
    let second_owner = root.scope().owned_scope();
    let current =
        second_owner.completion_once(unwind_safe(move |_: i32| called_in_callback.set(true)));

    drop(stale);
    assert!(current.submit(1));
    assert!(called.get());

    second_owner.dispose();
    drop(current);
    drop(second_owner);
    drop(first_owner);
    root.dispose().expect("root cleanup");
}
