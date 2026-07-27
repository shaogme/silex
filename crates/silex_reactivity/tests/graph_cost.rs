//! 图操作的采样探针：memo 重算、传播扇出、effect 扇出、深链销毁。
//!
//! 与 `tests/read_cost.rs` 同一个定位 —— 不是基准框架，只是让几个关键数字在
//! `cargo test --release -- --nocapture` 下可见。补这一组的直接原因是审计报告
//! §5 记了两轮的缺口（“没有基准”），以及阶段三方案 B 预测的“memo 重算会变慢
//! 3~8 ns/节点”需要一个能验证它的探针 —— 读路径的探针对求值路径一个字都
//! 说不出来。
//!
//! # 怎么读这些数字
//!
//! 每组给的都是**边际**成本：规模 k 的一整轮减去规模 1 的那一轮，再除以 k-1。
//! 这样写入本身、传播的固定开销、读取的固定开销全被减掉，剩下的就是
//! “每多一个节点，一次变更要多付多少”。绝对值受机器与构建影响，边际值稳定得多。
//!
//! debug 构建下的绝对数字没有参考价值（采样数也调小了）；Miri 下全部跳过。

use silex_reactivity::*;
use std::{
    cell::{Cell, RefCell},
    hint::black_box,
    rc::Rc,
    time::{Duration, Instant},
};

const DEBUG: bool = cfg!(debug_assertions);

/// 采样 `n` 轮，返回每轮耗时。
fn measure(n: usize, mut body: impl FnMut()) -> Duration {
    // 预热：第一轮要付 arena 建 chunk、`Vec` 首次扩容之类的一次性成本。
    for _ in 0..(n / 16).max(1) {
        body();
    }
    let t = Instant::now();
    for _ in 0..n {
        body();
    }
    t.elapsed() / n as u32
}

fn report(name: &str, per_round: Duration) {
    println!("{name:<30} {:>9.1} ns/轮", per_round.as_nanos() as f64);
}

/// 差分报告：`per_round` 减去 `base`，再摊到多出来的 `count` 个节点上。
fn report_marginal(name: &str, per_round: Duration, base: Duration, count: usize) {
    println!(
        "{name:<30} {:>9.1} ns/轮   边际 {:>7.2} ns/节点",
        per_round.as_nanos() as f64,
        per_round.saturating_sub(base).as_nanos() as f64 / count as f64
    );
}

/// 在一个 scope 里建一批节点，返回 scope 与 `f` 的产物。
///
/// 量完 `scope::dispose(scope)` 就能把这一批整个回收 —— 否则上一轮留下的
/// 订阅者还挂在图里，会把后面几轮的传播成本一起算进来。
fn scoped<T: 'static>(f: impl FnOnce() -> T + 'static) -> (ScopeId, T) {
    let out = Rc::new(RefCell::new(None));
    let o = out.clone();
    let scope = scope::create(move || *o.borrow_mut() = Some(f()));
    let value = out.borrow_mut().take().expect("scope 的闭包必然跑过");
    (scope, value)
}

// --- memo 重算 ---

/// 建一条 `source -> m1 -> ... -> mL` 的 memo 链，返回尾节点。
///
/// 每个 memo 都在上游基础上 +1，因此源头一变整条链的值都会变 ——
/// 相等性门控不会把传播挡在中途，链上每个节点每轮恰好重算一次。
/// 这条不变量由本文件的 `a_chain_recomputes_each_node_once_per_write` 钉住。
fn memo_chain(source: SignalId, len: usize) -> RawNodeId {
    assert!(len >= 1);
    let mut prev = source.to_raw();
    for _ in 0..len {
        let upstream = prev;
        prev =
            memo::create::<u64, _>(move |_| signal::get::<u64>(upstream).unwrap_or(0) + 1).to_raw();
    }
    prev
}

#[test]
#[cfg_attr(miri, ignore)]
fn probe_memo_recompute_cost() {
    let n = if DEBUG { 2_000 } else { 200_000 };

    let sample = |len: usize| -> Duration {
        let source = signal::create(0u64);
        let (scope, tail) = scoped(move || memo_chain(source, len));
        let d = measure(n, || {
            signal::update::<u64>(source, |v| *v = v.wrapping_add(1));
            black_box(signal::get::<u64>(tail));
        });
        scope::dispose(scope);
        scope::dispose(source);
        d
    };

    let base = sample(1);
    report("memo 链 L=1（写+求值+读）", base);
    for len in [2usize, 4, 8, 16] {
        report_marginal(&format!("memo 链 L={len}"), sample(len), base, len - 1);
    }
}

/// 干净的链上重复读取不驱动重算，一次写入让每个节点恰好重算一次。
///
/// 这是上面那组差分的正确性下限：少了它，`memo 链 L=16` 的数字可能是
/// “每轮重算 16 次”，也可能是“每轮重算 16×k 次”，两者的差分曲线长得一样。
#[test]
fn a_chain_recomputes_each_node_once_per_write() {
    const LEN: usize = 8;

    let source = signal::create(0u64);
    let runs = Rc::new(Cell::new(0usize));

    let mut prev = source.to_raw();
    for _ in 0..LEN {
        let upstream = prev;
        let r = runs.clone();
        prev = memo::create::<u64, _>(move |_| {
            r.set(r.get() + 1);
            signal::get::<u64>(upstream).unwrap_or(0) + 1
        })
        .to_raw();
    }
    let tail = prev;

    assert_eq!(signal::get::<u64>(tail), Some(LEN as u64));
    assert_eq!(runs.get(), LEN, "首次读取应当自上游向下游各算一次");

    for _ in 0..10 {
        assert_eq!(signal::get::<u64>(tail), Some(LEN as u64));
    }
    assert_eq!(runs.get(), LEN, "链干净时重复读取不得重算");

    signal::update::<u64>(source, |v| *v += 1);
    assert_eq!(signal::get::<u64>(tail), Some(LEN as u64 + 1));
    assert_eq!(runs.get(), 2 * LEN, "一次写入让链上每个节点恰好重算一次");
}

// --- 扇出 ---

/// 一个 signal 挂 k 个 memo 订阅者：写入 → 读遍全部。
///
/// 走的是完整的一轮（传播 BFS 标脏 + k 次求值 DFS + k 次重算），
/// 也就是扇出型 UI（一个 store 字段喂十几个派生表达式）的真实形态。
#[test]
#[cfg_attr(miri, ignore)]
fn probe_memo_fanout_cost() {
    let sample = |k: usize| -> Duration {
        let n = if DEBUG {
            (20_000 / k).max(50)
        } else {
            (2_000_000 / k).max(200)
        };
        let source = signal::create(0u64);
        let (scope, subs) = scoped(move || {
            (0..k)
                .map(|_| memo_chain(source, 1))
                .collect::<Vec<RawNodeId>>()
        });
        let d = measure(n, || {
            signal::update::<u64>(source, |v| *v = v.wrapping_add(1));
            for &m in &subs {
                black_box(signal::get::<u64>(m));
            }
        });
        scope::dispose(scope);
        scope::dispose(source);
        d
    };

    let base = sample(1);
    report("memo 扇出 k=1", base);
    for k in [10usize, 100, 1000] {
        report_marginal(&format!("memo 扇出 k={k}"), sample(k), base, k - 1);
    }
}

/// 扇出的对照组：k 个 memo，每个订阅**自己的** signal，一次 batch 全部写入。
///
/// 节点总数、重算次数、cache 足迹都与 [`probe_memo_fanout_cost`] 的 k 相同，
/// **唯一**的区别是每张订阅者表只有 1 条而不是 k 条。两组的边际曲线一比，
/// 就能把“扇出成本随 k 超线性增长”归因到订阅者表的长度上，
/// 而不是节点数量或 cache。
#[test]
#[cfg_attr(miri, ignore)]
fn probe_memo_fanout_cost_with_disjoint_sources() {
    let sample = |k: usize| -> Duration {
        let n = if DEBUG {
            (20_000 / k).max(50)
        } else {
            (2_000_000 / k).max(200)
        };
        let sources: Vec<SignalId> = (0..k).map(|_| signal::create(0u64)).collect();
        let srcs = sources.clone();
        let (scope, subs) = scoped(move || {
            srcs.iter()
                .map(|&s| memo_chain(s, 1))
                .collect::<Vec<RawNodeId>>()
        });
        let d = measure(n, || {
            scope::batch(|| {
                for &s in &sources {
                    signal::update::<u64>(s, |v| *v = v.wrapping_add(1));
                }
            });
            for &m in &subs {
                black_box(signal::get::<u64>(m));
            }
        });
        scope::dispose(scope);
        for s in sources {
            scope::dispose(s);
        }
        d
    };

    let base = sample(1);
    report("独立源 k=1", base);
    for k in [10usize, 100, 1000] {
        report_marginal(&format!("独立源 k={k}"), sample(k), base, k - 1);
    }
}

/// 同上，但订阅者是 effect：走推送调度那条路（传播入队 + flush 执行）。
#[test]
#[cfg_attr(miri, ignore)]
fn probe_effect_fanout_cost() {
    let sample = |k: usize| -> Duration {
        let n = if DEBUG {
            (20_000 / k).max(50)
        } else {
            (1_000_000 / k).max(200)
        };
        let source = signal::create(0u64);
        let hits = Rc::new(Cell::new(0u64));
        let h0 = hits.clone();
        let (scope, ()) = scoped(move || {
            for _ in 0..k {
                let h = h0.clone();
                effect::create(move || {
                    h.set(
                        h.get()
                            .wrapping_add(signal::get::<u64>(source).unwrap_or(0)),
                    );
                });
            }
        });
        let d = measure(n, || {
            signal::update::<u64>(source, |v| *v = v.wrapping_add(1));
        });
        black_box(hits.get());
        scope::dispose(scope);
        scope::dispose(source);
        d
    };

    let base = sample(1);
    report("effect 扇出 k=1", base);
    for k in [10usize, 100, 1000] {
        report_marginal(&format!("effect 扇出 k={k}"), sample(k), base, k - 1);
    }
}

// --- 销毁 ---

/// 深度嵌套的 scope 链，每层带一个 cleanup；只给销毁计时（建树在计时之外）。
///
/// 量的是后序工作栈本身的成本 —— AUDIT P19.8 把它从调用栈搬到了堆上，
/// 这里是它唯一的性能刻度。
///
/// # 深度为什么是 500 而不是 2000
///
/// 受限的是**建树**那一侧，不是销毁：`nest` 是真的在原生栈上递归，而嵌套一层
/// `scope::create` 现在要吃约 1.3 KB 原生栈（方案 B 之前约 0.4 KB —— 公开调用
/// 多了驱动层几个函数帧）。2 MB 的默认测试线程栈因此在约 1600 层就满了。
/// 销毁这一侧的栈深度仍然是**常数**，那条不变量由
/// `runtime::scope` 里的 `disposing_a_deep_tree_does_not_overflow_the_stack`
/// 覆盖 —— 它用迭代的方式建五万层，正是为了不受这一条影响。
#[test]
#[cfg_attr(miri, ignore)]
fn probe_deep_dispose_cost() {
    fn nest(depth: usize, hits: &Rc<Cell<usize>>) {
        let h = hits.clone();
        scope::on_cleanup(move || h.set(h.get() + 1));
        if depth > 0 {
            let hits = hits.clone();
            scope::create(move || nest(depth - 1, &hits));
        }
    }

    let (depth, rounds) = if DEBUG { (200, 20) } else { (500, 400) };
    let hits = Rc::new(Cell::new(0usize));
    let mut total = Duration::ZERO;

    for _ in 0..rounds {
        let h = hits.clone();
        let root = scope::create(move || nest(depth - 1, &h));
        let t = Instant::now();
        scope::dispose(root);
        total += t.elapsed();
    }

    assert_eq!(hits.get(), depth * rounds, "每层的 cleanup 都该恰好跑一次");
    println!(
        "{:<30} {:>9.1} ns/节点   （深度 {depth}）",
        "深链销毁",
        total.as_nanos() as f64 / (depth * rounds) as f64
    );
}
