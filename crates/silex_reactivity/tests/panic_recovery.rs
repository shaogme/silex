//! 用户代码 panic 之后运行时必须仍然可用（AUDIT P2 的承诺，二轮 §2.5 补齐）。

use silex_reactivity::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

/// 在预期会 panic 的代码块期间静默 panic 输出。
fn silently<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    result
}

/// memo 的计算闭包 panic：节点被标回 `Dirty`，下一次读取会重算一遍，
/// 而不是拿着一份 panic 点之前收集的残缺依赖表假装自己是干净的。
#[test]
fn a_panicking_memo_recomputes_on_the_next_read() {
    let a = signal(1i32);
    let b = signal(1i32);
    // memo 是**立即**首算的，所以第一次不能炸，否则 panic 逃到构造点之外。
    let explode = Rc::new(Cell::new(false));
    let runs = Rc::new(Cell::new(0));

    let (e, r) = (explode.clone(), runs.clone());
    let m = memo::<i32, _>(move |_| {
        r.set(r.get() + 1);
        let first = try_get_signal::<i32>(a).unwrap();
        if e.get() {
            panic!("boom");
        }
        // panic 的那一轮跑不到这里，b 因此没有被登记成依赖。
        first + try_get_signal::<i32>(b).unwrap()
    });
    assert_eq!(try_get_signal::<i32>(m), Some(2));

    // 让重算在半途 panic。
    explode.set(true);
    let _ = silently(|| {
        update_signal::<i32>(a, |v| *v = 2);
        try_get_signal::<i32>(m)
    });
    let after_panic = runs.get();

    // 关键：节点被标回 `Dirty`，所以下一次读取真的重算，而不是把
    // “运行前置的 Clean + 半截依赖表” 当成一个有效状态用下去。
    explode.set(false);
    assert_eq!(
        try_get_signal::<i32>(m),
        Some(3),
        "被 panic 打断的 memo 必须在下一次读取时重算"
    );
    assert!(runs.get() > after_panic);

    // 重算跑完了，b 这一次被登记上，因此写 b 会让它再次失效。
    update_signal::<i32>(b, |v| *v = 10);
    assert_eq!(try_get_signal::<i32>(m), Some(12));
}

/// 依赖成环时 `evaluate` 会 panic，工作栈必须由守卫归还 —— 池子被掏空之后
/// 运行时照样能继续正常工作。
#[test]
fn the_runtime_still_works_after_a_cycle_panic() {
    let s = signal(0i32);
    let second: Rc<Cell<Option<NodeId>>> = Rc::new(Cell::new(None));

    let second_c = second.clone();
    let first = memo(move |_: Option<&i32>| {
        let _ = try_get_signal::<i32>(s);
        match second_c.get() {
            Some(other) => try_get_signal::<i32>(other).unwrap_or(0),
            None => 0,
        }
    });
    let other = memo(move |_: Option<&i32>| try_get_signal::<i32>(first).unwrap_or(0) + 1);
    second.set(Some(other));

    update_signal(s, |v: &mut i32| *v += 1);
    let _ = try_get_signal::<i32>(first);

    // 环成型，反复触发若干次 panic：每一次都会经过 evaluate 的工作栈借还。
    for _ in 0..8 {
        let _ = silently(|| {
            update_signal(s, |v: &mut i32| *v += 1);
            try_get_signal::<i32>(first)
        });
    }

    // 一条与环完全无关的链，必须照常工作。
    let fresh = signal(10i32);
    let doubled = memo::<i32, _>(move |_| try_get_signal::<i32>(fresh).unwrap() * 2);
    let seen = Rc::new(Cell::new(0));
    let sc = seen.clone();
    effect(move || sc.set(try_get_signal::<i32>(doubled).unwrap()));

    assert_eq!(seen.get(), 20);
    update_signal::<i32>(fresh, |v| *v = 21);
    assert_eq!(seen.get(), 42, "环 panic 之后运行时必须仍然可用");
}

/// effect panic 之后：计算闭包被放回、重入锁被释放、已登记的依赖仍然有效。
///
/// 注意契约的边界 —— panic 点**之后**本该读到的依赖没有被登记，effect 对它们
/// 不会有反应。这一条是有意保留的（见 `NodeRunGuard::drop` 的注释）：
/// 把 effect 标脏却不重新入队会让它彻底失联，而重新入队等于 panic 自动重试。
#[test]
fn a_panicking_effect_still_reacts_to_its_registered_dependencies() {
    let registered = signal(0i32);
    let never_read = signal(0i32);
    let explode = Rc::new(Cell::new(false));
    let runs = Rc::new(Cell::new(0));

    let (e, r) = (explode.clone(), runs.clone());
    effect(move || {
        let _ = try_get_signal::<i32>(registered);
        r.set(r.get() + 1);
        if e.get() {
            panic!("boom");
        }
        let _ = try_get_signal::<i32>(never_read);
    });

    assert_eq!(runs.get(), 1);

    explode.set(true);
    let _ = silently(|| update_signal::<i32>(registered, |v| *v = 1));
    assert_eq!(runs.get(), 2, "panic 的那一次也算跑过");

    // 闭包被放回、重入锁释放：已登记的依赖照常唤醒它。
    explode.set(false);
    update_signal::<i32>(registered, |v| *v = 2);
    assert_eq!(runs.get(), 3, "panic 不应让 effect 的计算闭包永久丢失");

    // 这一次跑完了，never_read 被重新登记上。
    update_signal::<i32>(never_read, |v| *v = 1);
    assert_eq!(runs.get(), 4);
}

/// 调度标志在 panic 之后必须复位：batch 深度、队列标志、追踪上下文。
#[test]
fn scheduler_flags_are_restored_after_a_panic() {
    let s = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(s);
        r.set(r.get() + 1);
    });

    // batch 里 panic：深度必须恢复，否则之后所有更新被永久挂起。
    let _ = silently(|| {
        batch(|| {
            update_signal::<i32>(s, |v| *v = 1);
            panic!("boom");
        })
    });

    update_signal::<i32>(s, |v| *v = 2);
    assert!(runs.get() >= 2, "batch 里的 panic 不应让调度永久停摆");

    // untrack 里 panic：追踪必须恢复（否则展开之后 observer 永久停在 None）。
    let t = signal(0i32);
    let _ = silently(|| effect(|| untrack(|| panic!("boom"))));

    let t2_runs = Rc::new(Cell::new(0));
    let tr2 = t2_runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(t);
        tr2.set(tr2.get() + 1);
    });
    assert_eq!(t2_runs.get(), 1);
    update_signal::<i32>(t, |v| *v = 1);
    assert_eq!(t2_runs.get(), 2, "untrack 里的 panic 不应让追踪永久关闭");
}

/// signal 的值在 update 闭包 panic 时必须被放回节点（守卫的职责）。
#[test]
fn a_signal_value_is_returned_when_the_update_closure_panics() {
    let s = signal(7i32);

    let _ = silently(|| {
        update_signal::<i32>(s, |v| {
            *v = 42;
            panic!("boom");
        })
    });

    assert_eq!(
        try_get_signal::<i32>(s),
        Some(42),
        "闭包已经改过的值必须随守卫放回节点"
    );

    // 节点仍然可写。
    update_signal::<i32>(s, |v| *v = 1);
    assert_eq!(try_get_signal::<i32>(s), Some(1));
}
