use silex_reactivity::{
    Callback, Derived, Effect, Memo, NodeRef, ReactiveError, ReadSignal, Runtime, StoredValue,
    WriteSignal,
};
use std::{
    cell::Cell,
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
        let _effect = scope.effect(move || {
            seen_in_effect.set(tail_in_effect.get());
        });

        assert_eq!(seen.get(), 3);
        set_source.set(4);
        assert_eq!(seen.get(), 6);
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
        let switch_in_effect = switch;
        let left_in_effect = left;
        let right_in_effect = right;
        let _effect = scope.effect(move || {
            runs_in_effect.set(runs_in_effect.get() + 1);
            seen_in_effect.set(if switch_in_effect.get() {
                left_in_effect.get()
            } else {
                right_in_effect.get()
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
        let _effect = scope.effect(move || {
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
        let _second = scope.effect(move || {
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
fn panic_in_update_restores_the_value_and_runtime() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (signal, set_signal) = scope.signal(1i32);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            set_signal.update(|_| panic!("update panic"));
        }));
        assert!(panic.is_err());
        assert_eq!(signal.get(), 1);
    });

    runtime.run(|scope| {
        let (signal, set_signal) = scope.signal(1i32);
        set_signal.set(2);
        assert_eq!(signal.get(), 2);
    });
}

#[test]
fn effect_cleanup_runs_before_the_next_run() {
    let mut runtime = Runtime::new();
    let cleanups = Rc::new(Cell::new(0));
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(0i32);
        let scope_for_effect = *scope;
        let cleanups_in_effect = cleanups.clone();
        let _effect = scope.effect(move || {
            let _ = source.get();
            let cleanups = cleanups_in_effect.clone();
            scope_for_effect.on_cleanup(move || cleanups.set(cleanups.get() + 1));
        });
        assert_eq!(cleanups.get(), 0);
        set_source.set(1);
        assert_eq!(cleanups.get(), 1);
    });
    assert_eq!(cleanups.get(), 2);
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

    // A node that is no longer owned cannot be reconstructed through a raw id;
    // the public API has no raw-id constructor or implicit runtime fallback.
    assert_eq!(
        ReactiveError::NoSuchNode.to_string(),
        "节点不存在或所属 scope 已结束"
    );
}

#[test]
fn epoch_memo_fast_path_skips_evaluation_when_upstream_unchanged() {
    let mut runtime = Runtime::new();
    runtime.run(|scope| {
        let (source, set_source) = scope.signal(10i32);

        let m1_runs = Rc::new(Cell::new(0));
        let m1_runs_cb = m1_runs.clone();
        // M1: source / 10 (当 source 在 10..19 变动时，M1 结果始终为 1)
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

        // 初始求值
        assert_eq!(m1.get(), 1);
        assert_eq!(m2.get(), 101);
        assert_eq!(m3.get(), 202);
        assert_eq!(m1_runs.get(), 1);
        assert_eq!(m2_runs.get(), 1);
        assert_eq!(m3_runs.get(), 1);

        // 修改 source 从 10 变为 15：M1 评估后发现 15 / 10 = 1，值没有变化！
        set_source.set(15);

        // 读取 M3，验证 M3 和 M2 借助 Epoch Fast-Path 直接 0 次闭包计算命中跳过！
        assert_eq!(m3.get(), 202);
        assert_eq!(m2.get(), 101);
        assert_eq!(m1.get(), 1);

        // M1 仅重新评估了一次，而 M2 和 M3 的闭包运行次数依然保持为 1！完全被极速跳过！
        assert_eq!(m1_runs.get(), 2);
        assert_eq!(m2_runs.get(), 1);
        assert_eq!(m3_runs.get(), 1);

        // 当修改 source 变为 20 时：M1 计算结果变为 2，值发生改变！
        set_source.set(20);
        assert_eq!(m3.get(), 204);
        assert_eq!(m1_runs.get(), 3);
        assert_eq!(m2_runs.get(), 2);
        assert_eq!(m3_runs.get(), 2);
    });
}

#[test]
fn track_batch_works_in_scoped_runtime() {
    use silex_reactivity::track_batch;

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
