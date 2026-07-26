//! 别名探针：专门把运行时里那些“把内部引用交给用户闭包”的路径跑一遍。
//!
//! 这些用例本身断言的东西很少 —— 它们的价值全在 `cargo miri test` 下：
//! CI 一直在跑 Miri，但从来没跑到危险路径上，因为
//!
//! - `signal::track_batch`（→ `Runtime::track_dependencies`）在本 crate 的测试里
//!   一次都没被调用过（只有 `silex_core` 在用，而它不在 Miri 名单里）；
//! - `store::try_update` / `signal::try_with` 的既有用例，闭包里都没有重入
//!   运行时，因此触发不了别名冲突。
//!
//! 也就是说 Miri 覆盖率此前给的是一种虚假的安全感。下面每个用例都对应一条
//! “运行时持有指向某张 `SparseSecondaryMap` 的引用期间，用户代码又动了同一张表”
//! 的路径 —— 在 Stacked Borrows 下，这不需要 key 冲突，动同一张 map 就够了。

use silex_reactivity::*;
use std::{cell::Cell, rc::Rc};

/// `track_dependencies` 在循环里持续持有 owner 节点的 `&mut`，同时反复访问
/// 同一张 `reactive` 表去取每个 target。
#[test]
fn batch_tracking_does_not_alias_the_owner_node() {
    let a = signal::create(1i32);
    let b = signal::create(2i32);
    let c = signal::create(3i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect::create(move || {
        // 一次批量登记三个依赖：owner 只查一次，target 查三次。
        signal::track_batch(&[a.raw(), b.raw(), c.raw()]);
        r.set(r.get() + 1);
    });

    assert_eq!(runs.get(), 1);

    signal::update::<i32>(a, |v| *v += 1);
    assert_eq!(runs.get(), 2, "批量登记的依赖必须真的能触发重跑");

    signal::update::<i32>(c, |v| *v += 1);
    assert_eq!(runs.get(), 3);
}

/// 批量登记里混入自引用与已销毁的句柄：两条 `continue` 分支同样要跑到。
#[test]
fn batch_tracking_tolerates_self_and_dead_handles() {
    let alive = signal::create(1i32);
    let dead = signal::create(2i32);
    scope::dispose(dead);

    let runs = Rc::new(Cell::new(0));
    let r = runs.clone();
    effect::create(move || {
        signal::track_batch(&[dead.raw(), alive.raw(), dead.raw()]);
        r.set(r.get() + 1);
    });

    signal::update::<i32>(alive, |v| *v += 1);
    assert_eq!(runs.get(), 2);
}

/// `store::try_update` 把 `&mut T` 直接交给用户闭包，闭包里再读一个
/// signal —— 读会走 `reactive` 表，写会走 `extras` 表，两条都要探。
#[test]
fn a_stored_value_update_may_touch_the_runtime_from_inside() {
    let other = signal::create(7i32);
    let cfg = store::create(0i32);

    let out = store::try_update::<i32, _>(cfg, |c| {
        // 重入运行时：这一步在 Stacked Borrows 下会作废 `c` 的来源指针。
        *c = signal::try_get::<i32>(other).unwrap();
        // 作废之后继续用 `c`。
        *c += 1;
        *c
    });

    assert_eq!(out, Ok(8));
    assert_eq!(store::try_with(cfg, |v: &i32| *v), Ok(8));
}

/// 同上，但重入的是**同一张 `extras` 表**：读一个别的 stored value、再往
/// 表里插一个新条目（`insert` 会取 `&mut Vec`，是最容易作废外层借用的一步）。
#[test]
fn a_stored_value_update_may_touch_the_same_map() {
    let a = store::create(1i32);
    let b = store::create(10i32);

    let out = store::try_update::<i32, _>(a, |x| {
        *x += store::try_with(b, |v: &i32| *v).unwrap();
        // 往同一张表里插新条目（可能触发 chunk 扩容）。
        let fresh = store::create(100i32);
        *x += store::try_with(fresh, |v: &i32| *v).unwrap();
        *x
    });

    assert_eq!(out, Ok(111));
}

/// `signal::try_with` 把 `&T` 交给用户闭包，闭包里写另一个 signal。
#[test]
fn reading_a_signal_may_write_another_one_from_inside() {
    let a = signal::create(1i32);
    let b = signal::create(100i32);

    let out = signal::try_with::<i32, _>(a, |v| {
        // 写 b 会走一整套 “取出值 → 调 updater → 放回 → 传播” 的流程，
        // 期间同一张 `reactive` 表被 `get_mut` 反复访问 —— 而 `v` 正是从这张表
        // 里借出来的。
        signal::update::<i32>(b, |x| *x += 1);
        // 往同一张表里插新条目。
        let fresh = signal::create(0i32);
        signal::update::<i32>(fresh, |x| *x = 1);
        *v
    });

    assert_eq!(out, Ok(1));
    assert_eq!(signal::try_get::<i32>(b), Ok(101));
}

/// `signal::try_with` 的闭包里创建新节点：`graph` / `node_aux` / `reactive`
/// 三张表都会被写。
#[test]
fn reading_a_signal_may_create_nodes_from_inside() {
    let a = signal::create(1i32);

    let out = signal::try_with::<i32, _>(a, |v| {
        let fresh = signal::create(*v * 2);
        signal::try_get::<i32>(fresh).unwrap()
    });

    assert_eq!(out, Ok(2));
}

/// effect 体内做完整的一轮读写：这条路径同时压到 `track_dependency`、
/// `update_if_necessary` 与 `commit_update`。
#[test]
fn a_memo_chain_survives_a_full_propagation_round() {
    let src = signal::create(1i32);
    let doubled = memo::create::<i32, _>(move |_| signal::try_get::<i32>(src).unwrap() * 2);
    let plus_one = memo::create::<i32, _>(move |_| signal::try_get::<i32>(doubled).unwrap() + 1);

    let seen = Rc::new(Cell::new(0));
    let s = seen.clone();
    effect::create(move || s.set(signal::try_get::<i32>(plus_one).unwrap()));

    assert_eq!(seen.get(), 3);
    signal::update::<i32>(src, |v| *v = 10);
    assert_eq!(seen.get(), 21);
}
