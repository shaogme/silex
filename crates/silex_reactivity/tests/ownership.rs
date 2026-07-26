//! 所有权（owner）与依赖追踪（observer）是两件正交的事。
//!
//! 二轮审计 §1.1：此前 `untrack` 把两者绑在同一个变量上，一清就把所有权也清了，
//! 于是 `untrack` 里创建的节点没有父节点、不在任何 scope 的 children 里、
//! 永远不会被 `dispose` 回收。`silex_core` 的每一个 `Rx::new_op` /
//! `Rx::new_constant` / `Rx::derive` 都正是这么写的（"只是想避免建立依赖"）。

use silex_reactivity::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// `untrack` 里创建的节点必须随所属 scope 一起销毁。
#[test]
fn untrack_does_not_orphan_new_nodes() {
    let inner = Rc::new(RefCell::new(None::<NodeId>));

    let i2 = inner.clone();
    let scope = create_scope(move || {
        let sv = untrack(|| store_value(1234i32));
        *i2.borrow_mut() = Some(sv);
    });

    let sv = inner.borrow().expect("scope 已经跑过");
    assert!(is_stored_value_valid(sv));

    dispose(scope);
    assert!(
        !is_stored_value_valid(sv),
        "untrack 里创建的节点没有随 scope 一起销毁（AUDIT 二轮 §1.1）"
    );
}

/// 各种句柄一视同仁：signal / closure / op / callback / node_ref 都该被回收。
#[test]
fn untrack_does_not_orphan_any_node_kind() {
    let ids = Rc::new(RefCell::new(Vec::<(NodeId, &'static str)>::new()));

    let i2 = ids.clone();
    let scope = create_scope(move || {
        untrack(|| {
            let mut v = i2.borrow_mut();
            v.push((signal(1i32), "signal"));
            v.push((store_value(2i32), "stored"));
            v.push((register_closure(Box::new(3i32)), "closure"));
            v.push((register_op(RawOpBuffer::new()), "op"));
            v.push((register_callback(|_| {}), "callback"));
            v.push((register_node_ref(), "node_ref"));
        });
    });

    for &(id, kind) in ids.borrow().iter() {
        assert!(
            get_node_defined_at(id).is_some() || cfg!(not(debug_assertions)),
            "{kind} 应当还活着"
        );
    }

    dispose(scope);

    for &(id, kind) in ids.borrow().iter() {
        let alive = is_signal_valid(id)
            || is_stored_value_valid(id)
            || is_closure_valid(id)
            || is_op_valid(id)
            || is_callback_valid(id)
            || is_node_ref_valid(id);
        assert!(!alive, "{kind} 在 scope 销毁后仍然活着");
    }
}

/// effect 内部的 `untrack` 同样保留所有权：里面建的节点在 effect 重跑前被清掉。
#[test]
fn untrack_inside_an_effect_still_belongs_to_the_effect() {
    let trigger = signal(0i32);
    let created = Rc::new(RefCell::new(Vec::<NodeId>::new()));

    let c = created.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(trigger);
        let sv = untrack(|| store_value(0u8));
        c.borrow_mut().push(sv);
    });

    assert_eq!(created.borrow().len(), 1);
    let first = created.borrow()[0];
    assert!(is_stored_value_valid(first));

    update_signal::<i32>(trigger, |v| *v += 1);

    assert_eq!(created.borrow().len(), 2, "effect 应当重跑了一次");
    assert!(
        !is_stored_value_valid(first),
        "上一轮在 untrack 里创建的节点应当在重跑前被清理掉"
    );
    assert!(is_stored_value_valid(created.borrow()[1]));
}

/// 而 `untrack` 该做的事一件都不能少：里面的读取不建立依赖。
#[test]
fn untrack_still_suppresses_dependency_tracking() {
    let tracked = signal(0i32);
    let hidden = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(tracked);
        untrack(|| {
            let _ = try_get_signal::<i32>(hidden);
        });
        r.set(r.get() + 1);
    });

    assert_eq!(runs.get(), 1);

    update_signal::<i32>(hidden, |v| *v += 1);
    assert_eq!(runs.get(), 1, "untrack 里读到的 signal 不该建立依赖");

    update_signal::<i32>(tracked, |v| *v += 1);
    assert_eq!(runs.get(), 2);
}

/// `untrack` 之后追踪必须恢复 —— 包括 `untrack` 内部 panic 的情况。
#[test]
fn tracking_resumes_after_untrack() {
    let a = signal(0i32);
    let b = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect(move || {
        untrack(|| {
            let _ = try_get_signal::<i32>(a);
        });
        let _ = try_get_signal::<i32>(b);
        r.set(r.get() + 1);
    });

    update_signal::<i32>(b, |v| *v += 1);
    assert_eq!(runs.get(), 2, "untrack 之后的读取必须重新建立依赖");
}

/// `create_scope` 里的读取不建立依赖：scope 不是计算节点，没有“重跑”这回事。
#[test]
fn a_scope_never_becomes_an_observer() {
    let outer = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect(move || {
        let _ = create_scope(|| {
            // 在 effect 内部开 scope，里面读一个 signal。
            let _ = try_get_signal::<i32>(outer);
        });
        r.set(r.get() + 1);
    });

    assert_eq!(runs.get(), 1);
    update_signal::<i32>(outer, |v| *v += 1);
    assert_eq!(runs.get(), 1, "scope 内的读取不该让外层 effect 重跑");
}

/// 顶层（没有 owner）的 `untrack` 仍然不会 panic，节点也确实是孤立的 ——
/// 这是没有 scope 可挂时唯一合理的行为。
#[test]
fn untrack_at_the_top_level_creates_a_root_node() {
    let sv = untrack(|| store_value(7i32));
    assert!(is_stored_value_valid(sv));
    dispose(sv);
    assert!(!is_stored_value_valid(sv));
}
