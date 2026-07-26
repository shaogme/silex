//! 阶段三（内部架构重写 · 方案 A）的行为固化。
//!
//! 这一轮把 `ReactiveNode` 的元数据换成 `Cell`、载荷换成 `RefCell`，把两张表
//! 的引用 provenance 从“承载 chunk 的那个 `Vec`”改成“chunk 自己的分配”，
//! 并顺手删掉了图算法那一层只有一个实现者的抽象。公开 API 一个字都没动，
//! 因此这里断言的是**它买回来的东西**：
//!
//! 1. 从前只能靠注释维系的“不要在读的时候重入运行时”，现在是一条能报出来的
//!    诊断（[`ReactiveError::Reentrant`]）而不是静默的别名违规；
//! 2. 依赖扫描带游标之后，“一个 memo 依赖十几个上游 memo”仍然每个只算一次；
//! 3. 借出中的值 / 闭包遇上“节点在闭包里把自己销毁了”时不写野指针，
//!    该析构的照常析构。

use silex_reactivity::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn silently<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    result
}

// --- 1. 读路径上的重入现在说得出话 ---

/// `signal::try_get` 会在持有节点值借用的情况下调用 `T::clone`（crate 文档里
/// 唯一剩下的“靠约定维系”的规则）。阶段三把值放进 `RefCell` 之后，一个会
/// 反过来**写同一个 signal** 的 `Clone` 实现拿到的是一句明确的诊断：
/// debug 构建下断言失败，release 下 [`ReactiveError::Reentrant`]。
///
/// 从前它是静默的：写入走 `SparseSecondaryMap::with_mut`，在 Stacked Borrows
/// 下直接作废读取侧手里那个引用，然后接着 `clone` 下去。
#[test]
fn a_clone_that_writes_the_same_signal_is_reported_not_silently_aliased() {
    thread_local! {
        static TARGET: Cell<Option<SignalId>> = const { Cell::new(None) };
        static INNER: Cell<Option<ReactiveResult<()>>> = const { Cell::new(None) };
    }

    #[derive(PartialEq, Debug)]
    struct Nosy(i32);

    impl Clone for Nosy {
        fn clone(&self) -> Self {
            if let Some(id) = TARGET.with(Cell::take) {
                INNER.with(|slot| slot.set(Some(signal::try_update(id, |v: &mut Nosy| v.0 += 1))));
            }
            Self(self.0)
        }
    }

    let s = signal::create(Nosy(1));
    TARGET.with(|slot| slot.set(Some(s)));

    let outer = silently(|| signal::try_get::<Nosy>(s));

    if cfg!(debug_assertions) {
        assert!(outer.is_err(), "debug 构建下重入写入必须触发断言");
    } else {
        // 读取本身照常完成，重入的那次写入被拒绝。
        assert_eq!(outer.unwrap(), Ok(Nosy(1)));
        assert_eq!(INNER.with(Cell::take), Some(Err(ReactiveError::Reentrant)));
    }

    // 无论哪条分支，运行时都还活着，值也没有被改坏。
    assert_eq!(signal::try_get::<Nosy>(s), Ok(Nosy(1)));
}

/// 在 `T::clone` 里**读**同一个 signal 是允许的 —— 借用是共享的。
#[test]
fn a_clone_that_reads_the_same_signal_is_fine() {
    thread_local! {
        static TARGET: Cell<Option<SignalId>> = const { Cell::new(None) };
        static SEEN: Cell<bool> = const { Cell::new(false) };
    }

    #[derive(PartialEq, Debug)]
    struct Curious(i32);

    impl Clone for Curious {
        fn clone(&self) -> Self {
            if let Some(id) = TARGET.with(Cell::take) {
                SEEN.with(|slot| slot.set(signal::try_get::<Curious>(id).is_ok()));
            }
            Self(self.0)
        }
    }

    let s = signal::create(Curious(5));
    TARGET.with(|slot| slot.set(Some(s)));

    assert_eq!(signal::try_get::<Curious>(s), Ok(Curious(5)));
    assert!(SEEN.with(Cell::get), "嵌套的只读读取应当成功");
}

// --- 2. 依赖扫描的游标（§2.2） ---

/// 一个 memo 依赖十几个上游 memo：一次上游变更之后，每个节点仍然恰好重算一次。
///
/// 从前 `evaluate` 每次回到这个节点都要把整张依赖表重新填进一个 `Vec` 再从头
/// 扫一遍（O(k²) 次比较 + k+1 次整表拷贝）。改成带游标的增量扫描之后，语义
/// 必须一字不差 —— 这条用例数的就是“语义”那一半。
#[test]
fn a_wide_memo_recomputes_every_upstream_exactly_once() {
    const WIDTH: usize = 16;

    let root = signal::create(0i32);
    let counts: Vec<Rc<Cell<usize>>> = (0..WIDTH).map(|_| Rc::new(Cell::new(0))).collect();

    let uppers: Vec<MemoId> = counts
        .iter()
        .enumerate()
        .map(|(i, count)| {
            let count = count.clone();
            memo::create::<i32, _>(move |_| {
                count.set(count.get() + 1);
                signal::try_get::<i32>(root).unwrap_or(0) + i as i32
            })
        })
        .collect();

    let sink_runs = Rc::new(Cell::new(0));
    let sink_runs_c = sink_runs.clone();
    let uppers_c = uppers.clone();
    let sink = memo::create::<i32, _>(move |_| {
        sink_runs_c.set(sink_runs_c.get() + 1);
        uppers_c
            .iter()
            .map(|&m| signal::try_get::<i32>(m).unwrap_or(0))
            .sum()
    });

    // 首次读取把整棵树算出来：每层各一次。
    let first = signal::try_get::<i32>(sink).unwrap();
    assert_eq!(first, (0..WIDTH as i32).sum::<i32>());
    assert!(counts.iter().all(|c| c.get() == 1));
    assert_eq!(sink_runs.get(), 1);

    // 一次上游写入 → 每个上游 memo 各重算一次，sink 也只重算一次。
    signal::update::<i32>(root, |v| *v = 10);
    let second = signal::try_get::<i32>(sink).unwrap();
    assert_eq!(second, (0..WIDTH as i32).map(|i| i + 10).sum::<i32>());

    for (i, c) in counts.iter().enumerate() {
        assert_eq!(c.get(), 2, "第 {i} 个上游 memo 应当恰好重算一次");
    }
    assert_eq!(sink_runs.get(), 2, "汇聚节点也只该重算一次");
}

// --- 3. “节点在自己的闭包里被销毁” ---

/// 值被借出交给用户闭包期间，闭包把这个节点销毁了：守卫归还时查无此节点，
/// 值随守卫一起析构 —— 既不写野指针，也不泄漏。
#[test]
fn destroying_a_node_from_inside_its_own_update_closure_drops_the_value() {
    let drops = Rc::new(Cell::new(0));

    struct Spy(Rc<Cell<usize>>);
    impl Drop for Spy {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let s = signal::create(Spy(drops.clone()));
    let result = signal::try_update(s, |_: &mut Spy| {
        scope::dispose(s);
    });

    assert_eq!(result, Ok(()));
    assert_eq!(drops.get(), 1, "借出中的值必须恰好析构一次");
    assert!(!s.is_alive());
    // 运行时仍然可用。
    let fresh = signal::create(1i32);
    assert_eq!(signal::try_get::<i32>(fresh), Ok(1));
}

/// 同样的事发生在非响应式载荷上（`store` / `callback` / `node_ref` 的底座）。
#[test]
fn destroying_a_stored_value_from_inside_its_own_closure_drops_the_payload() {
    let drops = Rc::new(Cell::new(0));

    struct Spy(Rc<Cell<usize>>);
    impl Drop for Spy {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let sv = store::create(Spy(drops.clone()));
    let result = store::try_update(sv, |_: &mut Spy| {
        scope::dispose(sv);
    });

    assert_eq!(result, Ok(()));
    assert_eq!(drops.get(), 1);
    assert!(!sv.is_alive());
}

/// effect 在自己的体内销毁自己：计算闭包归还时节点已经没了，闭包随守卫析构。
#[test]
fn an_effect_may_dispose_itself_from_its_own_body() {
    let dropped = Rc::new(Cell::new(false));

    struct Spy(Rc<Cell<bool>>);
    impl Drop for Spy {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }

    let slot: Rc<Cell<Option<EffectId>>> = Rc::new(Cell::new(None));
    let slot_c = slot.clone();
    let spy = Spy(dropped.clone());

    let e = effect::create(move || {
        // 捕获 `spy`，这样闭包被析构时我们看得见。
        let _ = &spy;
        if let Some(me) = slot_c.get() {
            scope::dispose(me);
        }
    });
    slot.set(Some(e));
    assert!(!dropped.get());

    // 再触发一次运行：这次它会把自己销毁。
    let trigger = signal::create(0i32);
    let _ = signal::try_get::<i32>(trigger);
    scope::dispose(e);

    assert!(!e.is_alive());
    assert!(dropped.get(), "effect 的计算闭包必须被析构");
}

// --- 4. scope 销毁之后不留活节点 ---

/// scope 销毁后，它下面的每一种节点都真的没了（`untrack` 里创建的也一样，
/// 见 AUDIT 二轮 §1.1）—— 阶段三改了每一张表的分配方式，这条不变量得重测。
#[test]
fn disposing_a_scope_reclaims_every_kind_of_child() {
    type AliveProbes = Rc<RefCell<Vec<Box<dyn Fn() -> bool>>>>;
    let handles: AliveProbes = Rc::new(RefCell::new(Vec::new()));
    let h = handles.clone();

    let root = scope::create(move || {
        let s = signal::create(1i32);
        let m = memo::create::<i32, _>(move |_| signal::try_get::<i32>(s).unwrap_or(0));
        let e = effect::create(move || {
            let _ = signal::try_get::<i32>(m);
        });
        let sv = scope::untrack(|| store::create(7u8));
        let cb = callback::create(|_| {});
        let nr = node_ref::create::<u32>();

        let mut h = h.borrow_mut();
        h.push(Box::new(move || s.is_alive()));
        h.push(Box::new(move || m.is_alive()));
        h.push(Box::new(move || e.is_alive()));
        h.push(Box::new(move || sv.is_alive()));
        h.push(Box::new(move || cb.is_alive()));
        h.push(Box::new(move || nr.is_alive()));
    });

    assert!(handles.borrow().iter().all(|alive| alive()));
    scope::dispose(root);
    for (i, alive) in handles.borrow().iter().enumerate() {
        assert!(!alive(), "第 {i} 个子节点没有随 scope 一起销毁");
    }
    assert!(!root.is_alive());
}
