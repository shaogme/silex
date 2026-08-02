use silex_reactivity::{CompletionToken, Runtime};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn root_stays_active_until_explicit_dispose() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let setter = Rc::new(RefCell::new(None));
    let setter_for_mount = setter.clone();
    let seen_for_mount = seen.clone();

    let mut root = runtime.run(move |scope| {
        let (value, set_value) = scope.signal(0i32);
        let seen = seen_for_mount.clone();
        scope.effect(move || seen.set(value.get()));
        *setter_for_mount.borrow_mut() = Some(set_value);
    });

    assert_eq!(seen.get(), 0);
    setter
        .borrow()
        .as_ref()
        .expect("root setter should be registered")
        .set(3);
    assert_eq!(seen.get(), 3);

    root.dispose().expect("root disposal should succeed");
    assert!(!root.is_active());
    setter
        .borrow()
        .as_ref()
        .expect("root setter should remain a safe invalid capability")
        .set(4);
    assert_eq!(seen.get(), 3);
}

#[test]
fn root_completion_is_invalidated_by_dispose() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));
    let token_slot = Rc::new(RefCell::new(None::<CompletionToken<i32>>));
    let token_slot_for_mount = token_slot.clone();
    let seen_for_mount = seen.clone();

    let mut root = runtime.run(move |scope| {
        let seen = seen_for_mount.clone();
        let token = scope.completion(move |value: i32| seen.set(value));
        *token_slot_for_mount.borrow_mut() = Some(token);
    });

    let token = token_slot
        .borrow()
        .as_ref()
        .expect("root token should be registered")
        .clone();
    assert!(token.submit(7));
    assert_eq!(seen.get(), 7);

    root.dispose().expect("root disposal should succeed");
    assert!(!token.submit(8));
    assert_eq!(seen.get(), 7);
}

#[test]
fn root_cleanup_runs_once_on_drop() {
    let cleaned = Rc::new(Cell::new(0));
    {
        let mut runtime = Runtime::new();
        let cleaned_for_mount = cleaned.clone();
        let _root = runtime.run(move |scope| {
            scope.on_cleanup(move || cleaned_for_mount.set(cleaned_for_mount.get() + 1));
        });
    }
    assert_eq!(cleaned.get(), 1);
}

#[test]
fn root_dispose_runs_host_cancellation_hooks() {
    let cancelled = Rc::new(Cell::new(0));
    let cancelled_for_mount = cancelled.clone();
    let mut runtime = Runtime::new();
    let mut root = runtime.run(move |scope| {
        scope.on_dispose(move || {
            cancelled_for_mount.set(cancelled_for_mount.get() + 1);
        });
    });

    root.dispose().expect("root disposal should succeed");
    assert_eq!(cancelled.get(), 1);
    root.dispose()
        .expect("repeated root disposal should be inert");
    assert_eq!(cancelled.get(), 1);
}

#[test]
fn root_dispose_continues_after_cancellation_panic() {
    let completed = Rc::new(Cell::new(false));
    let completed_for_mount = completed.clone();
    let mut runtime = Runtime::new();
    let mut root = runtime.run(move |scope| {
        scope.on_dispose(|| panic!("cancel panic"));
        scope.on_dispose(move || completed_for_mount.set(true));
    });

    assert!(root.dispose().is_err());
    assert!(completed.get());
}

#[test]
fn runtime_rejects_run_while_root_is_active() {
    let mut runtime = Runtime::new();
    let mut root = runtime.run(|_| {});

    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime.run(|_| {});
    }));
    assert!(panic.is_err());

    root.dispose().expect("root disposal should succeed");
    runtime.run(|scope| {
        let (value, set_value) = scope.signal(1);
        set_value.set(2);
        assert_eq!(value.get(), 2);
    });
}

#[test]
fn run_callback_panic_wins_over_cleanup_panic() {
    let mut runtime = Runtime::new();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime.run(|scope| {
            scope.on_cleanup(|| panic!("cleanup panic"));
            panic!("run callback panic");
        });
    }))
    .expect_err("run callback should panic");

    assert_eq!(panic.downcast_ref::<&str>(), Some(&"run callback panic"));
    let _root = runtime.run(|_| {});
}

#[test]
fn explicit_dispose_reports_cleanup_panic() {
    let mut runtime = Runtime::new();
    let mut root = runtime.run(|scope| {
        scope.on_cleanup(|| panic!("cleanup panic"));
    });

    assert!(root.dispose().is_err());
    assert!(!root.is_active());
}
