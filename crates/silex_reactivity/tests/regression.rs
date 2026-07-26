//! 代码审查（AUDIT_silex_reactivity.md）中每一个可复现缺陷的回归测试。
//!
//! 这些场景在修复前**全部**不在测试覆盖范围内，而它们描述的都是最常见的
//! 真实用法：在 effect 里初始化派生状态、effect 里 panic、级联更新。

use silex_reactivity::*;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

/// 在预期会 panic 的代码块期间静默 panic 输出，避免测试日志里出现误导性的 backtrace。
fn silently<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    result
}

// --- P1: effect 首次运行中写 signal，不得丢失订阅 ---

#[test]
fn effect_writing_a_signal_on_its_first_run_keeps_every_subscription() {
    let s = signal(0i32);
    let other = signal(100i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(s);
        let _ = try_get_signal::<i32>(other); // 与写入完全无关的第二个依赖
        runs_c.set(runs_c.get() + 1);
        if runs_c.get() == 1 {
            update_signal(s, |v: &mut i32| *v = 1);
        }
    });

    let after_create = runs.get();
    assert!(after_create >= 1, "effect 至少应该跑过一次");

    update_signal(other, |v: &mut i32| *v += 1);
    assert!(
        runs.get() > after_create,
        "对无关依赖 `other` 的订阅必须保留下来"
    );

    let after_other = runs.get();
    update_signal(s, |v: &mut i32| *v += 1);
    assert!(
        runs.get() > after_other,
        "对自己写过的依赖 `s` 的订阅也必须保留下来"
    );
}

#[test]
fn cleanup_does_not_fire_in_the_middle_of_the_run_that_registered_it() {
    let s = signal(0i32);
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let first = Rc::new(Cell::new(true));

    let log_c = log.clone();
    let first_c = first.clone();
    effect(move || {
        log_c.borrow_mut().push("body-start");

        let log_cleanup = log_c.clone();
        on_cleanup(move || log_cleanup.borrow_mut().push("cleanup"));

        let _ = try_get_signal::<i32>(s);
        if first_c.replace(false) {
            update_signal(s, |v: &mut i32| *v = 1);
        }

        log_c.borrow_mut().push("body-end");
    });

    let log = log.borrow();
    assert_eq!(
        &log[..2],
        &["body-start", "body-end"],
        "本次运行注册的 cleanup 不能在本次 body 结束前被调用，实际日志：{log:?}"
    );
}

// --- P2: 用户代码 panic 不得让运行时永久停摆 ---

#[test]
fn a_panicking_effect_does_not_stop_the_scheduler() {
    let s = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(s);
        runs_c.set(runs_c.get() + 1);
        if runs_c.get() == 2 {
            panic!("boom");
        }
    });

    let result = silently(|| update_signal(s, |v: &mut i32| *v = 1));
    assert!(result.is_err(), "panic 应该向调用方传播");
    assert_eq!(runs.get(), 2);

    // 关键断言：一个全新的、与出事 effect 完全无关的 effect 仍然会被调度。
    let other = signal(0i32);
    let other_runs = Rc::new(Cell::new(0));
    let other_runs_c = other_runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(other);
        other_runs_c.set(other_runs_c.get() + 1);
    });
    assert_eq!(other_runs.get(), 1);

    update_signal(other, |v: &mut i32| *v = 1);
    assert_eq!(
        other_runs.get(),
        2,
        "`running_queue` 卡在 true 会让整个响应式系统静默停摆"
    );
}

#[test]
fn a_panic_inside_batch_does_not_wedge_the_batch_depth() {
    let s = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(s);
        runs_c.set(runs_c.get() + 1);
    });
    assert_eq!(runs.get(), 1);

    let result = silently(|| {
        batch(|| {
            update_signal(s, |v: &mut i32| *v = 1);
            panic!("boom");
        })
    });
    assert!(result.is_err());

    // `batch_depth` 卡在 1 会让此后所有更新被永久挂起。
    update_signal(s, |v: &mut i32| *v = 2);
    assert!(runs.get() > 1, "batch panic 之后更新必须继续生效");
}

#[test]
fn a_panicking_effect_keeps_its_computation() {
    let s = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(s);
        runs_c.set(runs_c.get() + 1);
        if runs_c.get() == 2 {
            panic!("boom");
        }
    });

    let _ = silently(|| update_signal(s, |v: &mut i32| *v = 1));
    assert_eq!(runs.get(), 2);

    // 计算闭包在 panic 展开时被放回，effect 之后依然可以重跑。
    update_signal(s, |v: &mut i32| *v = 2);
    assert_eq!(runs.get(), 3, "panic 不应让 effect 的计算闭包永久丢失");
}

// --- P8: 计算期间产生的失效不得被覆盖 ---

#[test]
fn updates_written_during_a_computation_are_not_lost() {
    let s = signal(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect(move || {
        let v = try_get_signal::<i32>(s).unwrap_or(0);
        runs_c.set(runs_c.get() + 1);
        if (1..5).contains(&v) {
            update_signal(s, |x: &mut i32| *x += 1);
        }
    });

    update_signal(s, |v: &mut i32| *v = 1);

    assert_eq!(try_get_signal::<i32>(s), Some(5), "级联更新应该一路跑到 5");
    assert!(
        runs.get() >= 5,
        "effect 必须观察到自己写入的每一个值，实际只跑了 {} 次",
        runs.get()
    );
}

// --- P15: 调度时机不得取决于入口路径 ---

#[test]
fn scheduling_order_is_the_same_on_the_first_run_and_on_re_runs() {
    let trigger = signal(0i32);
    let target = signal(0i32);
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    // 观察者：订阅 target。
    let log_b = log.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(target);
        log_b.borrow_mut().push("observer");
    });

    // 生产者：在自己体内写 target。
    let log_a = log.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(trigger);
        log_a.borrow_mut().push("producer-start");
        update_signal(target, |v: &mut i32| *v += 1);
        log_a.borrow_mut().push("producer-end");
    });

    let first_run: Vec<&'static str> = log.borrow()[1..].to_vec();
    log.borrow_mut().clear();

    update_signal(trigger, |v: &mut i32| *v += 1);
    let re_run: Vec<&'static str> = log.borrow().clone();

    assert_eq!(
        first_run, re_run,
        "同一段用户代码的执行顺序不能取决于它是首跑还是重跑"
    );
    assert_eq!(
        first_run,
        vec!["producer-start", "producer-end", "observer"],
        "观察者必须被推迟到生产者结束之后，而不是在它中途嵌套执行"
    );
}

// --- P4: `List::Many` 路径（≥2 个订阅者 / 依赖）在 Miri 下必须干净 ---

#[test]
fn many_subscribers_and_many_dependencies() {
    let sources: Vec<NodeId> = (0..8).map(signal).collect();
    let hits = Rc::new(Cell::new(0));

    // 8 个 effect × 8 个依赖：订阅者列表与依赖列表都会走 `ThinVec` 分支。
    for _ in 0..8 {
        let sources = sources.clone();
        let hits_c = hits.clone();
        effect(move || {
            let sum: i32 = sources
                .iter()
                .map(|&id| try_get_signal::<i32>(id).unwrap_or(0))
                .sum();
            hits_c.set(hits_c.get() + sum as usize + 1);
        });
    }

    for &id in &sources {
        update_signal(id, |v: &mut i32| *v += 1);
    }

    assert!(hits.get() > 0);

    // 逐个退订：触发 `Many -> Single -> Empty` 的降级路径。
    for &id in &sources {
        dispose(id);
    }
}

// --- P5: update 闭包在借用作用域之外通知下游 ---

#[test]
fn downstream_effects_run_after_the_update_closure_returns() {
    let s = signal(0i32);
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    let log_c = log.clone();
    effect(move || {
        let _ = try_get_signal::<i32>(s);
        log_c.borrow_mut().push("effect");
    });
    log.borrow_mut().clear();

    let log_c = log.clone();
    update_signal(s, move |v: &mut i32| {
        *v += 1;
        log_c.borrow_mut().push("closure");
    });

    assert_eq!(
        *log.borrow(),
        vec!["closure", "effect"],
        "下游 effect 必须在 update 闭包返回之后才执行"
    );
}
