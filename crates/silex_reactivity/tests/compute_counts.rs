//! “一次变更，每个 memo 至多算一次”的计数断言。
//!
//! 二轮审计 §1.2：这个 crate 一直没有人数过 memo 到底被计算了几次，于是
//! “动态发现一个正处于 Dirty 的上游依赖 → 本节点被自己的依赖标脏 → 出栈后
//! 状态仍是 Dirty → 下游读它时再算一遍” 这条路径静静地把每层 memo 的计算
//! 次数翻了一倍。

use silex_reactivity::*;
use std::{cell::Cell, rc::Rc};

fn counter() -> (Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let c = Rc::new(Cell::new(0));
    (c.clone(), c)
}

/// §6.2 的复现：memo 在自己的计算过程中**第一次**读到一个 Dirty 的上游 memo。
#[test]
fn a_memo_is_computed_once_per_change() {
    let flag = signal::create(false);
    let a = signal::create(0i32);
    let m1 = memo::create::<i32, _>(move |_| signal::try_get::<i32>(a).unwrap() + 100);

    let (runs, r) = counter();
    let m2 = memo::create::<i32, _>(move |_| {
        r.set(r.get() + 1);
        if signal::try_get::<bool>(flag).unwrap() {
            signal::try_get::<i32>(m1).unwrap()
        } else {
            -1
        }
    });
    effect::create(move || {
        let _ = signal::try_get::<i32>(m2);
    });

    let base = runs.get();
    assert_eq!(base, 1, "首算一次");

    // m1 变脏，此时 m2 还没有依赖它。
    signal::update::<i32>(a, |v| *v = 1);
    assert_eq!(runs.get(), base, "m2 还没依赖 m1，不该被这次写入惊动");

    // m2 重算，过程中第一次读到 Dirty 的 m1。
    let _ = signal::set_if_changed(flag, true);
    assert_eq!(
        runs.get(),
        base + 1,
        "一次变更只该让 m2 计算一次（AUDIT 二轮 §1.2）"
    );

    assert_eq!(signal::try_get::<i32>(m2), Ok(101));
}

/// 同一条路径在链上不该逐层放大。
#[test]
fn dynamically_discovered_dirty_dependencies_do_not_multiply_along_a_chain() {
    let flag = signal::create(false);
    let src = signal::create(0i32);

    // 一条 4 层的上游链，全部先变脏。
    let l1 = memo::create::<i32, _>(move |_| signal::try_get::<i32>(src).unwrap() + 1);
    let l2 = memo::create::<i32, _>(move |_| signal::try_get::<i32>(l1).unwrap() + 1);
    let l3 = memo::create::<i32, _>(move |_| signal::try_get::<i32>(l2).unwrap() + 1);
    let l4 = memo::create::<i32, _>(move |_| signal::try_get::<i32>(l3).unwrap() + 1);

    let (runs, r) = counter();
    let sink = memo::create::<i32, _>(move |_| {
        r.set(r.get() + 1);
        if signal::try_get::<bool>(flag).unwrap() {
            signal::try_get::<i32>(l4).unwrap()
        } else {
            -1
        }
    });
    effect::create(move || {
        let _ = signal::try_get::<i32>(sink);
    });

    let base = runs.get();
    signal::update::<i32>(src, |v| *v = 10);
    let _ = signal::set_if_changed(flag, true);

    assert_eq!(runs.get(), base + 1);
    assert_eq!(signal::try_get::<i32>(sink), Ok(14));
}

/// 菱形依赖：共同的下游只该算一次，不该看到中间态。
#[test]
fn a_diamond_recomputes_its_sink_once() {
    let src = signal::create(1i32);
    let left = memo::create::<i32, _>(move |_| signal::try_get::<i32>(src).unwrap() * 2);
    let right = memo::create::<i32, _>(move |_| signal::try_get::<i32>(src).unwrap() * 3);

    let (runs, r) = counter();
    let sink = memo::create::<i32, _>(move |_| {
        r.set(r.get() + 1);
        signal::try_get::<i32>(left).unwrap() + signal::try_get::<i32>(right).unwrap()
    });
    effect::create(move || {
        let _ = signal::try_get::<i32>(sink);
    });

    assert_eq!(runs.get(), 1);
    assert_eq!(signal::try_get::<i32>(sink), Ok(5));

    signal::update::<i32>(src, |v| *v = 2);
    assert_eq!(runs.get(), 2, "菱形的下游一次变更只算一次");
    assert_eq!(signal::try_get::<i32>(sink), Ok(10));
}

/// 条件订阅：分支没走到的那条依赖变化时不该重算。
#[test]
fn a_conditional_memo_only_recomputes_for_the_branch_it_took() {
    let use_a = signal::create(true);
    let a = signal::create(1i32);
    let b = signal::create(100i32);

    let (runs, r) = counter();
    let m = memo::create::<i32, _>(move |_| {
        r.set(r.get() + 1);
        if signal::try_get::<bool>(use_a).unwrap() {
            signal::try_get::<i32>(a).unwrap()
        } else {
            signal::try_get::<i32>(b).unwrap()
        }
    });
    effect::create(move || {
        let _ = signal::try_get::<i32>(m);
    });

    assert_eq!(runs.get(), 1);

    // 没被读到的分支变化：不重算。
    signal::update::<i32>(b, |v| *v = 200);
    assert_eq!(runs.get(), 1);

    // 被读到的分支变化：重算一次。
    signal::update::<i32>(a, |v| *v = 2);
    assert_eq!(runs.get(), 2);

    // 切换分支：重算一次，之后 a 的变化不再影响它。
    let _ = signal::set_if_changed(use_a, false);
    assert_eq!(runs.get(), 3);
    assert_eq!(signal::try_get::<i32>(m), Ok(200));

    signal::update::<i32>(a, |v| *v = 3);
    assert_eq!(runs.get(), 3, "切换分支后 a 应该已经被退订");
}

/// 一次 batch 里写多个上游，共同的下游仍然只算一次。
#[test]
fn batched_writes_recompute_a_shared_sink_once() {
    let a = signal::create(1i32);
    let b = signal::create(2i32);

    let (runs, r) = counter();
    let sum = memo::create::<i32, _>(move |_| {
        r.set(r.get() + 1);
        signal::try_get::<i32>(a).unwrap() + signal::try_get::<i32>(b).unwrap()
    });
    effect::create(move || {
        let _ = signal::try_get::<i32>(sum);
    });

    assert_eq!(runs.get(), 1);

    scope::batch(|| {
        signal::update::<i32>(a, |v| *v = 10);
        signal::update::<i32>(b, |v| *v = 20);
    });

    assert_eq!(runs.get(), 2);
    assert_eq!(signal::try_get::<i32>(sum), Ok(30));
}

/// effect 也一样：一次变更只跑一遍。
#[test]
fn an_effect_runs_once_per_change_even_with_a_dynamic_dependency() {
    let flag = signal::create(false);
    let a = signal::create(0i32);
    let m = memo::create::<i32, _>(move |_| signal::try_get::<i32>(a).unwrap() + 100);

    let (runs, r) = counter();
    effect::create(move || {
        r.set(r.get() + 1);
        if signal::try_get::<bool>(flag).unwrap() {
            let _ = signal::try_get::<i32>(m);
        }
    });

    let base = runs.get();
    signal::update::<i32>(a, |v| *v = 1);
    assert_eq!(runs.get(), base, "effect 还没依赖 m");

    let _ = signal::set_if_changed(flag, true);
    assert_eq!(runs.get(), base + 1, "一次变更只该让 effect 跑一次");
}
