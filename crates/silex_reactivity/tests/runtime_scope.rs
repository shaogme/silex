use silex_reactivity::{ReactiveError, Runtime};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn runtime_run_provides_scoped_signal_and_effect() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (count, set_count) = scope.signal(0i32);
        let doubled = scope.memo(move |_| count.get() * 2);
        let runs_in_effect = runs.clone();
        let doubled_in_effect = doubled;
        let _effect = scope.effect(move || {
            let _ = doubled_in_effect.get();
            runs_in_effect.set(runs_in_effect.get() + 1);
        });

        assert_eq!(runs.get(), 1);
        set_count.set(1);
        assert_eq!(doubled.get(), 2);
        assert_eq!(runs.get(), 2);
    });

    assert_eq!(runs.get(), 2);
}

#[test]
fn non_static_effect_can_capture_data_and_scoped_signal() {
    let mut runtime = Runtime::new();
    let external = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (signal, set_signal) = scope.signal(1i32);
        let external_in_effect = external.clone();
        scope.effect(move || {
            external_in_effect.set(external_in_effect.get() + signal.get());
        });
        set_signal.set(2);
    });

    assert_eq!(external.get(), 3);
}

#[test]
fn child_scope_is_lexical_and_cleans_up_its_nodes() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        scope.scope(|child| {
            let (local, set_local) = child.signal(0i32);
            let runs = cleaned.clone();
            let _effect = child.effect(move || {
                let _ = local.get();
                runs.set(runs.get() + 1);
            });
            set_local.set(1);
            assert_eq!(cleaned.get(), 2);
        });
        assert_eq!(cleaned.get(), 2);
    });
}

#[test]
fn child_effect_reacts_to_parent_signal_and_detaches_on_exit() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (parent, set_parent) = scope.signal(0i32);
        scope.scope(|child| {
            let runs_in_effect = runs.clone();
            child.effect(move || {
                let _ = parent.get();
                runs_in_effect.set(runs_in_effect.get() + 1);
            });
            assert_eq!(runs.get(), 1);
            set_parent.set(1);
            assert_eq!(runs.get(), 2);
        });

        set_parent.set(2);
        assert_eq!(runs.get(), 2);
    });
}

#[test]
fn root_cleanup_runs_when_run_ends() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(false));
    let cleaned_in_scope = cleaned.clone();
    runtime.run(|scope| {
        scope.on_cleanup(move || cleaned_in_scope.set(true));
        assert!(!cleaned.get());
    });
    assert!(cleaned.get());
}

#[test]
fn cleanup_order_follows_lexical_scope_order() {
    let mut runtime = Runtime::new();
    let events = Rc::new(RefCell::new(Vec::new()));

    runtime.run(|scope| {
        let parent_events = events.clone();
        scope.on_cleanup(move || parent_events.borrow_mut().push("parent"));

        scope.scope(|child| {
            let child_events = events.clone();
            child.on_cleanup(move || child_events.borrow_mut().push("child"));
        });

        assert_eq!(events.borrow().as_slice(), &["child"]);
    });

    assert_eq!(events.borrow().as_slice(), &["child", "parent"]);
}

#[test]
fn child_scope_panic_cleans_up_before_parent_continues() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(false));
    let parent_continued = Rc::new(Cell::new(false));

    runtime.run(|scope| {
        let cleaned_in_child = cleaned.clone();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            scope.scope(|child| {
                child.on_cleanup(move || cleaned_in_child.set(true));
                panic!("child callback panic");
            });
        }));

        assert!(panic.is_err());
        assert!(cleaned.get());
        parent_continued.set(true);
    });

    assert!(parent_continued.get());
}

#[test]
fn child_callback_panic_is_not_replaced_by_cleanup_panic() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let panic = catch_unwind(AssertUnwindSafe(|| {
            scope.scope(|child| {
                child.on_cleanup(|| panic!("cleanup panic"));
                panic!("callback panic");
            });
        }))
        .expect_err("child callback should panic");

        assert_eq!(panic.downcast_ref::<&str>(), Some(&"callback panic"));
    });
}

#[test]
fn parent_effect_tracks_reads_inside_child_callback() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let parent_scope = *scope;
        let runs_in_effect = runs.clone();
        scope.effect(move || {
            parent_scope.scope(|_| {
                let _ = source.get();
            });
            runs_in_effect.set(runs_in_effect.get() + 1);
        });

        assert_eq!(runs.get(), 1);
        set_source.set(1);
        assert_eq!(runs.get(), 2);
    });
}

#[test]
fn child_local_signal_does_not_keep_parent_effect_queued_after_exit() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let parent_scope = *scope;
        let runs_in_effect = runs.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            scope.effect(move || {
                runs_in_effect.set(runs_in_effect.get() + 1);
                parent_scope.scope(|child| {
                    let (local, set_local) = child.signal(0i32);
                    let _ = local.get();
                    set_local.set(1);
                });
            });
        }));

        assert!(result.is_ok());
        assert_eq!(runs.get(), 1);
    });
}

#[test]
fn cleanup_can_reenter_an_active_parent_scope() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let seen_in_effect = seen.clone();
        scope.effect(move || seen_in_effect.set(source.get()));

        scope.scope(|child| {
            child.on_cleanup(move || set_source.set(1));
        });

        assert_eq!(seen.get(), 1);
    });
}

#[test]
fn panic_in_run_still_drops_the_root_scope() {
    let mut runtime = Runtime::new();
    let cleaned = Rc::new(Cell::new(false));
    let cleaned_in_scope = cleaned.clone();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime.run(|scope| {
            scope.on_cleanup(move || cleaned_in_scope.set(true));
            panic!("run panic");
        });
    }));
    assert!(panic.is_err());
    assert!(cleaned.get());

    runtime.run(|scope| {
        let (signal, set_signal) = scope.signal(1i32);
        set_signal.set(2);
        assert_eq!(signal.get(), 2);
    });
}

#[test]
fn cleanup_panic_does_not_poison_runtime() {
    let mut runtime = Runtime::new();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime.run(|scope| {
            scope.on_cleanup(|| panic!("cleanup panic"));
        });
    }));
    assert!(panic.is_err());

    runtime.run(|scope| {
        let (signal, set_signal) = scope.signal(1i32);
        set_signal.set(2);
        assert_eq!(signal.get(), 2);
    });
}

#[test]
fn cleanup_panic_does_not_skip_remaining_cleanups() {
    let mut runtime = Runtime::new();
    let remaining_cleanup_ran = Rc::new(Cell::new(false));
    let remaining_cleanup_ran_in_scope = remaining_cleanup_ran.clone();

    let panic = catch_unwind(AssertUnwindSafe(|| {
        runtime.run(|scope| {
            scope.on_cleanup(|| panic!("first cleanup panic"));
            scope.on_cleanup(move || remaining_cleanup_ran_in_scope.set(true));
        });
    }));

    assert!(panic.is_err());
    assert!(remaining_cleanup_ran.get());
}

#[test]
fn handles_are_invalid_after_their_scope_and_runtimes_are_isolated() {
    let mut first = Runtime::new();
    let mut second = Runtime::new();
    first.run(|scope| {
        let (signal, _) = scope.signal(1i32);
        assert!(signal.is_alive());
        second.run(|other| {
            let (other_signal, _) = other.signal(2i32);
            assert_eq!(other_signal.get(), 2);
            assert_eq!(signal.get(), 1);
        });
        assert!(signal.is_alive());
    });

    let mut gone = Runtime::new();
    gone.run(|scope| {
        let (signal, _) = scope.signal(1i32);
        assert!(signal.is_alive());
        let _ = signal;
    });

    assert_eq!(
        ReactiveError::NoSuchNode.to_string(),
        "节点不存在或所属 scope 已结束"
    );
}
