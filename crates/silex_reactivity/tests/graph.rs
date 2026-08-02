use silex_reactivity::{Memo, Runtime, notify, track_batch};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn memo_and_derived_keep_their_notification_rules() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let memo_runs = Rc::new(Cell::new(0));
        let memo_runs_in_callback = memo_runs.clone();
        let memo_source = source;
        let memo = scope.memo(move |_| {
            memo_runs_in_callback.set(memo_runs_in_callback.get() + 1);
            memo_source.get() / 10
        });
        let derived_runs = Rc::new(Cell::new(0));
        let derived_runs_in_callback = derived_runs.clone();
        let derived_source = source;
        let derived = scope.derived(move || {
            derived_runs_in_callback.set(derived_runs_in_callback.get() + 1);
            derived_source.get() / 10
        });

        assert_eq!(memo.get(), 0);
        assert_eq!(derived.get(), 0);
        set_source.set(2);
        assert_eq!(memo.get(), 0);
        assert_eq!(derived.get(), 0);
        assert_eq!(memo_runs.get(), 2);
        assert_eq!(derived_runs.get(), 2);
    });
}

#[test]
fn dependency_chain_evaluates_upstream_before_effect() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let middle_source = source;
        let middle = scope.memo(move |_| middle_source.get() + 1);
        let tail_source = middle;
        let tail = scope.memo(move |_| tail_source.get() + 1);
        let seen = Rc::new(Cell::new(0));
        let seen_in_effect = seen.clone();
        let tail_in_effect = tail;
        scope.effect(move || {
            seen_in_effect.set(tail_in_effect.get());
        });

        assert_eq!(seen.get(), 3);
        set_source.set(4);
        assert_eq!(seen.get(), 6);
    });
}

#[test]
fn diamond_dependencies_do_not_observe_intermediate_state() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(RefCell::new(Vec::new()));

    runtime.run(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let left = scope.memo(move |_| source.get() + 1);
        let right = scope.memo(move |_| source.get() + 10);
        let seen_in_effect = seen.clone();
        scope.effect(move || {
            let left_value = left.get();
            let right_value = right.get();
            seen_in_effect
                .borrow_mut()
                .push((left_value, right_value, left_value + right_value));
        });

        assert_eq!(seen.borrow().as_slice(), &[(2, 11, 13)]);
        set_source.set(2);
        assert_eq!(seen.borrow().as_slice(), &[(2, 11, 13), (3, 12, 15)]);
    });
}

#[test]
fn dynamic_dependencies_are_replaced_on_each_effect_run() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (switch, set_switch) = scope.signal(true);
        let (left, set_left) = scope.signal(0i32);
        let (right, set_right) = scope.signal(0i32);
        let runs = Rc::new(Cell::new(0));
        let seen = Rc::new(Cell::new(0));
        let runs_in_effect = runs.clone();
        let seen_in_effect = seen.clone();
        scope.effect(move || {
            runs_in_effect.set(runs_in_effect.get() + 1);
            seen_in_effect.set(if switch.get() {
                left.get()
            } else {
                right.get()
            });
        });

        set_right.set(1);
        assert_eq!(runs.get(), 1);
        set_left.set(2);
        assert_eq!(runs.get(), 2);
        assert_eq!(seen.get(), 2);
        set_switch.set(false);
        assert_eq!(runs.get(), 3);
        assert_eq!(seen.get(), 1);
        set_left.set(3);
        assert_eq!(runs.get(), 3);
        set_right.set(4);
        assert_eq!(runs.get(), 4);
        assert_eq!(seen.get(), 4);
    });
}

#[test]
fn batch_delays_effects_and_untrack_preserves_ownership_context() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let (hidden, set_hidden) = scope.signal(0i32);
        let seen = Rc::new(Cell::new(0));
        let seen_in_effect = seen.clone();
        let effect_source = source;
        let effect_hidden = hidden;
        scope.effect(move || {
            seen_in_effect.set(effect_source.get() + effect_hidden.get());
        });

        scope.batch(|| {
            set_source.set(1);
            set_hidden.set(2);
            assert_eq!(seen.get(), 0);
        });
        assert_eq!(seen.get(), 3);

        let tracked = Rc::new(Cell::new(0));
        let tracked_in_effect = tracked.clone();
        let second_source = source;
        let second_hidden = hidden;
        scope.effect(move || {
            tracked_in_effect.set(second_hidden.get());
            let _ = second_source.get();
        });
        set_hidden.set(4);
        assert_eq!(tracked.get(), 4);
        assert_eq!(scope.untrack(|| hidden.get()), 4);
        set_source.set(2);
        assert_eq!(tracked.get(), 4);
    });
}

#[test]
fn epoch_memo_fast_path_skips_evaluation_when_upstream_unchanged() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(10i32);

        let m1_runs = Rc::new(Cell::new(0));
        let m1_runs_cb = m1_runs.clone();
        let m1 = scope.memo(move |_| {
            m1_runs_cb.set(m1_runs_cb.get() + 1);
            source.get() / 10
        });

        let m2_runs = Rc::new(Cell::new(0));
        let m2_runs_cb = m2_runs.clone();
        let m2 = scope.memo(move |_| {
            m2_runs_cb.set(m2_runs_cb.get() + 1);
            m1.get() + 100
        });

        let m3_runs = Rc::new(Cell::new(0));
        let m3_runs_cb = m3_runs.clone();
        let m3 = scope.memo(move |_| {
            m3_runs_cb.set(m3_runs_cb.get() + 1);
            m2.get() * 2
        });

        assert_eq!(m3.get(), 202);
        assert_eq!(m1_runs.get(), 1);
        assert_eq!(m2_runs.get(), 1);
        assert_eq!(m3_runs.get(), 1);

        set_source.set(15);
        assert_eq!(m3.get(), 202);
        assert_eq!(m1_runs.get(), 2);
        assert_eq!(m2_runs.get(), 1);
        assert_eq!(m3_runs.get(), 1);

        set_source.set(20);
        assert_eq!(m3.get(), 204);
        assert_eq!(m1_runs.get(), 3);
        assert_eq!(m2_runs.get(), 2);
        assert_eq!(m3_runs.get(), 2);
    });
}

#[test]
fn track_batch_tracks_all_signals_in_one_scope() {
    let mut runtime = Runtime::new();
    let runs = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (sig1, set_sig1) = scope.signal(10i32);
        let (sig2, set_sig2) = scope.signal(20i32);
        let runs_in_effect = runs.clone();

        scope.effect(move || {
            track_batch(&[sig1, sig2]);
            runs_in_effect.set(runs_in_effect.get() + 1);
        });

        assert_eq!(runs.get(), 1);
        set_sig1.set(11);
        assert_eq!(runs.get(), 2);
        set_sig2.set(21);
        assert_eq!(runs.get(), 3);
    });
}

#[test]
fn notify_recomputes_after_silent_interior_mutation() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (source, set_source) = scope.signal(RefCell::new(0i32));
        let seen_in_effect = seen.clone();
        scope.effect(move || {
            seen_in_effect.set(source.with(|value| *value.borrow()));
        });

        source.with(|value| {
            *value.borrow_mut() = 1;
            notify(&set_source);
            assert_eq!(seen.get(), 0);
        });

        assert_eq!(seen.get(), 1);
    });
}

#[test]
fn cross_scope_computation_stack_includes_scope_identity() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(1i32);
        let parent_memo = scope.memo(move |_| source.get() + 1);

        scope.scope(|child| {
            let parent_memo_in_child = parent_memo;
            let child_memo = child.memo(move |_| parent_memo_in_child.get() + 1);
            let seen = Rc::new(Cell::new(0));
            let seen_in_effect = seen.clone();
            child.effect(move || seen_in_effect.set(child_memo.get()));

            assert_eq!(seen.get(), 3);
            set_source.set(2);
            assert_eq!(seen.get(), 4);
        });
    });
}

#[test]
fn cross_scope_derived_reacts_and_detaches_on_exit() {
    let mut runtime = Runtime::new();
    let seen = Rc::new(Cell::new(0));

    runtime.run(|scope| {
        let (source, set_source) = scope.signal(1i32);
        scope.scope(|child| {
            let child_source = source;
            let derived = child.derived(move || child_source.get() * 2);
            let seen_in_effect = seen.clone();
            child.effect(move || seen_in_effect.set(derived.get()));

            assert_eq!(seen.get(), 2);
            set_source.set(2);
            assert_eq!(seen.get(), 4);
        });

        set_source.set(3);
        assert_eq!(seen.get(), 4);
    });
}

#[test]
fn cyclic_memo_dependency_panics_without_poisoning_the_scheduler() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let first_slot: Rc<RefCell<Option<Memo<'_, '_, i32>>>> = Rc::new(RefCell::new(None));
        let second_slot: Rc<RefCell<Option<Memo<'_, '_, i32>>>> = Rc::new(RefCell::new(None));
        let (source, set_source) = scope.signal(0i32);

        let second_slot_in_first = second_slot.clone();
        let first = scope.memo(move |_| {
            let dependency = second_slot_in_first.borrow().as_ref().copied();
            source.get() + dependency.map(|memo| memo.get()).unwrap_or(0)
        });
        *first_slot.borrow_mut() = Some(first);

        let first_slot_in_second = first_slot.clone();
        let second = scope.memo(move |_| {
            let dependency = first_slot_in_second.borrow().as_ref().copied();
            source.get() + dependency.map(|memo| memo.get()).unwrap_or(0)
        });
        *second_slot.borrow_mut() = Some(second);

        set_source.set(1);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = first.get();
        }));
        assert!(panic.is_err());

        set_source.set(2);
        assert!(first.is_alive());
        assert!(second.is_alive());
    });
}
