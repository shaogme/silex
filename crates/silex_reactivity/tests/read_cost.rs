//! 读写路径的采样探针（AUDIT 二轮 §1.3、§6.3）。
//!
//! 这不是基准测试框架 —— 它只是让“读一次 signal 到底要多久”这个数字在
//! `cargo test --release -- --nocapture` 下可见，免得性能回退悄无声息地溜进来。
//! 真正的 criterion 基准是另一件事（见报告 §5）。
//!
//! debug 构建下的绝对数字没有参考价值，所以默认只打印不断言；Miri 下直接跳过。

use silex_reactivity::*;
use std::{hint::black_box, time::Instant};

const N: usize = if cfg!(debug_assertions) {
    50_000
} else {
    2_000_000
};

fn sample(name: &str, mut body: impl FnMut() -> u64) {
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..N {
        acc = acc.wrapping_add(body());
    }
    black_box(acc);
    let ns = t.elapsed().as_nanos() as f64 / N as f64;
    println!("{name:<28} {ns:>6.1} ns/次");
}

#[test]
#[cfg_attr(miri, ignore)]
fn probe_read_cost() {
    let s = signal::create(0u64);
    sample("无 owner 上下文读取", || {
        black_box(signal::try_get::<u64>(s).unwrap())
    });
    sample("untracked 读取", || {
        black_box(signal::try_get_untracked::<u64>(s).unwrap())
    });

    let inner = signal::create(0u64);
    let m = memo::create::<u64, _>(move |_| signal::try_get::<u64>(inner).unwrap());
    sample("干净 memo 读取", || {
        black_box(signal::try_get::<u64>(m).unwrap())
    });
}

#[test]
#[cfg_attr(miri, ignore)]
fn probe_tracked_read_cost() {
    let s = signal::create(0u64);
    // 在 effect 体内读：会走完整的 track_dependency（首次之后由
    // `last_tracked_by` 去重，但查表与比较照样要做）。
    effect::create(move || {
        sample("effect 内追踪读取", || {
            black_box(signal::try_get::<u64>(s).unwrap())
        });
    });
}

#[test]
#[cfg_attr(miri, ignore)]
fn probe_write_cost() {
    let s = signal::create(0u64);
    sample("0 订阅者写入", || {
        signal::update::<u64>(s, |v| *v = v.wrapping_add(1));
        0
    });
}

/// 读一个干净的普通 signal 不得驱动任何计算 —— 提前返回的正确性下限。
#[test]
fn reading_a_clean_signal_runs_no_computation() {
    use std::{cell::Cell, rc::Rc};

    let s = signal::create(1i32);
    let runs = Rc::new(Cell::new(0));
    let r = runs.clone();
    let m = memo::create::<i32, _>(move |_| {
        r.set(r.get() + 1);
        signal::try_get::<i32>(s).unwrap()
    });

    assert_eq!(runs.get(), 1);
    for _ in 0..100 {
        assert_eq!(signal::try_get::<i32>(m), Ok(1));
    }
    assert_eq!(runs.get(), 1, "读一个干净的 memo 不该重算");
}

/// 提前返回不能把队列里的待办饿死：读取仍然是一个 flush 出口。
#[test]
fn reading_still_flushes_a_pending_queue() {
    use std::{cell::Cell, rc::Rc};

    let trigger = signal::create(0i32);
    let unrelated = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let r = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(trigger);
        r.set(r.get() + 1);
    });
    assert_eq!(runs.get(), 1);

    // 在 batch 里写入：effect 入队但不执行。
    scope::batch(|| {
        signal::update::<i32>(trigger, |v| *v += 1);
        assert_eq!(runs.get(), 1, "batch 期间不该执行");
    });
    assert_eq!(runs.get(), 2, "batch 结束时 flush");

    // 读一个与队列无关的干净 signal，不该让队列里的东西丢失。
    let _ = signal::try_get::<i32>(unrelated);
    assert_eq!(runs.get(), 2);
}
