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
    let inner = Rc::new(RefCell::new(None::<StoredId>));

    let i2 = inner.clone();
    let scope = scope::create(move || {
        let sv = scope::untrack(|| store::create(1234i32));
        *i2.borrow_mut() = Some(sv);
    });

    let sv = inner.borrow().expect("scope 已经跑过");
    assert!(sv.is_alive());

    scope::dispose(scope);
    assert!(
        !sv.is_alive(),
        "untrack 里创建的节点没有随 scope 一起销毁（AUDIT 二轮 §1.1）"
    );
}

/// 各种句柄一视同仁：signal / memo / effect / stored / callback / node_ref
/// 都该被回收。
///
/// 每一种的存活判定现在都是同一个 [`Handle::is_alive`] —— 从前是六个内容
/// 一模一样的 `is_*_valid` 自由函数（审计报告 §3.1）。
#[test]
fn untrack_does_not_orphan_any_node_kind() {
    struct Kinds {
        signal: SignalId,
        memo: MemoId,
        derived: DerivedId,
        effect: EffectId,
        stored: StoredId,
        boxed: StoredId,
        callback: CallbackId,
        node_ref: NodeRefId,
    }

    let ids = Rc::new(RefCell::new(None::<Kinds>));

    let i2 = ids.clone();
    let scope = scope::create(move || {
        scope::untrack(|| {
            *i2.borrow_mut() = Some(Kinds {
                signal: signal::create(1i32),
                memo: memo::create::<i32, _>(|_| 1),
                derived: memo::derived(Box::new(|| 1i32)),
                effect: effect::create(|| {}),
                stored: store::create(2i32),
                boxed: store::create(Box::new(3i32) as Box<dyn std::any::Any>),
                callback: callback::create(|_| {}),
                node_ref: node_ref::create::<i32>(),
            });
        });
    });

    macro_rules! each {
        ($k:expr, $f:expr) => {{
            let k = $k;
            let f: fn(bool, &'static str) = $f;
            f(k.signal.is_alive(), "signal");
            f(k.memo.is_alive(), "memo");
            f(k.derived.is_alive(), "derived");
            f(k.effect.is_alive(), "effect");
            f(k.stored.is_alive(), "stored");
            f(k.boxed.is_alive(), "boxed stored");
            f(k.callback.is_alive(), "callback");
            f(k.node_ref.is_alive(), "node_ref");
        }};
    }

    {
        let borrowed = ids.borrow();
        let k = borrowed.as_ref().expect("scope 已经跑过");
        each!(k, |alive, kind| assert!(alive, "{kind} 应当还活着"));
    }

    scope::dispose(scope);

    let borrowed = ids.borrow();
    let k = borrowed.as_ref().expect("scope 已经跑过");
    each!(k, |alive, kind| assert!(
        !alive,
        "{kind} 在 scope 销毁后仍然活着"
    ));
}

/// effect 内部的 `untrack` 同样保留所有权：里面建的节点在 effect 重跑前被清掉。
#[test]
fn untrack_inside_an_effect_still_belongs_to_the_effect() {
    let trigger = signal::create(0i32);
    let created = Rc::new(RefCell::new(Vec::<StoredId>::new()));

    let c = created.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(trigger);
        let sv = scope::untrack(|| store::create(0u8));
        c.borrow_mut().push(sv);
    });

    assert_eq!(created.borrow().len(), 1);
    let first = created.borrow()[0];
    assert!(first.is_alive());

    signal::update::<i32>(trigger, |v| *v += 1);

    assert_eq!(created.borrow().len(), 2, "effect 应当重跑了一次");
    assert!(
        !first.is_alive(),
        "上一轮在 untrack 里创建的节点应当在重跑前被清理掉"
    );
    assert!(created.borrow()[1].is_alive());
}

/// 而 `untrack` 该做的事一件都不能少：里面的读取不建立依赖。
#[test]
fn untrack_still_suppresses_dependency_tracking() {
    let tracked = signal::create(0i32);
    let hidden = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(tracked);
        scope::untrack(|| {
            let _ = signal::try_get::<i32>(hidden);
        });
        r.set(r.get() + 1);
    });

    assert_eq!(runs.get(), 1);

    signal::update::<i32>(hidden, |v| *v += 1);
    assert_eq!(runs.get(), 1, "untrack 里读到的 signal 不该建立依赖");

    signal::update::<i32>(tracked, |v| *v += 1);
    assert_eq!(runs.get(), 2);
}

/// `untrack` 之后追踪必须恢复 —— 包括 `untrack` 内部 panic 的情况。
#[test]
fn tracking_resumes_after_untrack() {
    let a = signal::create(0i32);
    let b = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect::create(move || {
        scope::untrack(|| {
            let _ = signal::try_get::<i32>(a);
        });
        let _ = signal::try_get::<i32>(b);
        r.set(r.get() + 1);
    });

    signal::update::<i32>(b, |v| *v += 1);
    assert_eq!(runs.get(), 2, "untrack 之后的读取必须重新建立依赖");
}

/// `scope::create` 里的读取不建立依赖：scope 不是计算节点，没有“重跑”这回事。
#[test]
fn a_scope_never_becomes_an_observer() {
    let outer = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect::create(move || {
        let _ = scope::create(|| {
            // 在 effect 内部开 scope，里面读一个 signal。
            let _ = signal::try_get::<i32>(outer);
        });
        r.set(r.get() + 1);
    });

    assert_eq!(runs.get(), 1);
    signal::update::<i32>(outer, |v| *v += 1);
    assert_eq!(runs.get(), 1, "scope 内的读取不该让外层 effect 重跑");
}

/// detached scope 里的 `untrack` 仍然不会 panic，节点也确实归属于该 scope。
#[test]
fn untrack_in_a_detached_scope_creates_a_owned_node() {
    let (owner, sv) = scope::create_detached(|| scope::untrack(|| store::create(7i32)));
    assert!(sv.is_alive());
    scope::dispose(owner);
    assert!(!sv.is_alive());
}
