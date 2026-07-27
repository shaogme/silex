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
    let s = signal::create(0i32);
    let other = signal::create(100i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        let _ = signal::try_get::<i32>(other); // 与写入完全无关的第二个依赖
        runs_c.set(runs_c.get() + 1);
        if runs_c.get() == 1 {
            signal::update(s, |v: &mut i32| *v = 1);
        }
    });

    let after_create = runs.get();
    assert!(after_create >= 1, "effect 至少应该跑过一次");

    signal::update(other, |v: &mut i32| *v += 1);
    assert!(
        runs.get() > after_create,
        "对无关依赖 `other` 的订阅必须保留下来"
    );

    let after_other = runs.get();
    signal::update(s, |v: &mut i32| *v += 1);
    assert!(
        runs.get() > after_other,
        "对自己写过的依赖 `s` 的订阅也必须保留下来"
    );
}

#[test]
fn cleanup_does_not_fire_in_the_middle_of_the_run_that_registered_it() {
    let s = signal::create(0i32);
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let first = Rc::new(Cell::new(true));

    let log_c = log.clone();
    let first_c = first.clone();
    effect::create(move || {
        log_c.borrow_mut().push("body-start");

        let log_cleanup = log_c.clone();
        scope::on_cleanup(move || log_cleanup.borrow_mut().push("cleanup"));

        let _ = signal::try_get::<i32>(s);
        if first_c.replace(false) {
            signal::update(s, |v: &mut i32| *v = 1);
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
    let s = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        runs_c.set(runs_c.get() + 1);
        if runs_c.get() == 2 {
            panic!("boom");
        }
    });

    let result = silently(|| signal::update(s, |v: &mut i32| *v = 1));
    assert!(result.is_err(), "panic 应该向调用方传播");
    assert_eq!(runs.get(), 2);

    // 关键断言：一个全新的、与出事 effect 完全无关的 effect 仍然会被调度。
    let other = signal::create(0i32);
    let other_runs = Rc::new(Cell::new(0));
    let other_runs_c = other_runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(other);
        other_runs_c.set(other_runs_c.get() + 1);
    });
    assert_eq!(other_runs.get(), 1);

    signal::update(other, |v: &mut i32| *v = 1);
    assert_eq!(
        other_runs.get(),
        2,
        "`running_queue` 卡在 true 会让整个响应式系统静默停摆"
    );
}

#[test]
fn a_panic_inside_batch_does_not_wedge_the_batch_depth() {
    let s = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        runs_c.set(runs_c.get() + 1);
    });
    assert_eq!(runs.get(), 1);

    let result = silently(|| {
        scope::batch(|| {
            signal::update(s, |v: &mut i32| *v = 1);
            panic!("boom");
        })
    });
    assert!(result.is_err());

    // `batch_depth` 卡在 1 会让此后所有更新被永久挂起。
    signal::update(s, |v: &mut i32| *v = 2);
    assert!(runs.get() > 1, "batch panic 之后更新必须继续生效");
}

#[test]
fn a_panicking_effect_keeps_its_computation() {
    let s = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        runs_c.set(runs_c.get() + 1);
        if runs_c.get() == 2 {
            panic!("boom");
        }
    });

    let _ = silently(|| signal::update(s, |v: &mut i32| *v = 1));
    assert_eq!(runs.get(), 2);

    // 计算闭包在 panic 展开时被放回，effect 之后依然可以重跑。
    signal::update(s, |v: &mut i32| *v = 2);
    assert_eq!(runs.get(), 3, "panic 不应让 effect 的计算闭包永久丢失");
}

// --- P8: 计算期间产生的失效不得被覆盖 ---

#[test]
fn updates_written_during_a_computation_are_not_lost() {
    let s = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let v = signal::try_get::<i32>(s).unwrap_or(0);
        runs_c.set(runs_c.get() + 1);
        if (1..5).contains(&v) {
            signal::update(s, |x: &mut i32| *x += 1);
        }
    });

    signal::update(s, |v: &mut i32| *v = 1);

    assert_eq!(signal::try_get::<i32>(s), Ok(5), "级联更新应该一路跑到 5");
    assert!(
        runs.get() >= 5,
        "effect 必须观察到自己写入的每一个值，实际只跑了 {} 次",
        runs.get()
    );
}

// --- P15: 调度时机不得取决于入口路径 ---

#[test]
fn scheduling_order_is_the_same_on_the_first_run_and_on_re_runs() {
    let trigger = signal::create(0i32);
    let target = signal::create(0i32);
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    // 观察者：订阅 target。
    let log_b = log.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(target);
        log_b.borrow_mut().push("observer");
    });

    // 生产者：在自己体内写 target。
    let log_a = log.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(trigger);
        log_a.borrow_mut().push("producer-start");
        signal::update(target, |v: &mut i32| *v += 1);
        log_a.borrow_mut().push("producer-end");
    });

    let first_run: Vec<&'static str> = log.borrow()[1..].to_vec();
    log.borrow_mut().clear();

    signal::update(trigger, |v: &mut i32| *v += 1);
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

// --- P4: 多订阅者 / 多依赖路径在 Miri 下必须干净 ---

#[test]
fn many_subscribers_and_many_dependencies() {
    let (source_owner, sources) =
        scope::create_detached(|| (0..8).map(signal::create).collect::<Vec<SignalId>>());
    let hits = Rc::new(Cell::new(0));

    // 8 个 effect × 8 个依赖：依赖列表走 `ThinVec`，订阅者表覆盖多槽位退订。
    let sources_for_effects = sources.clone();
    let hits_for_effects = hits.clone();
    let (effect_owner, ()) = scope::create_detached(move || {
        for _ in 0..8 {
            let sources = sources_for_effects.clone();
            let hits_c = hits_for_effects.clone();
            effect::create(move || {
                let sum: i32 = sources
                    .iter()
                    .map(|&id| signal::try_get::<i32>(id).unwrap_or(0))
                    .sum();
                hits_c.set(hits_c.get() + sum as usize + 1);
            });
        }
    });

    for &id in &sources {
        signal::update(id, |v: &mut i32| *v += 1);
    }

    assert!(hits.get() > 0);

    // 逐个退订：覆盖 swap-remove 与反向槽位修正。
    scope::dispose(effect_owner);
    scope::dispose(source_owner);
}

// --- P5: update 闭包在借用作用域之外通知下游 ---

#[test]
fn downstream_effects_run_after_the_update_closure_returns() {
    let s = signal::create(0i32);
    let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    let log_c = log.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        log_c.borrow_mut().push("effect");
    });
    log.borrow_mut().clear();

    let log_c = log.clone();
    signal::update(s, move |v: &mut i32| {
        *v += 1;
        log_c.borrow_mut().push("closure");
    });

    assert_eq!(
        *log.borrow(),
        vec!["closure", "effect"],
        "下游 effect 必须在 update 闭包返回之后才执行"
    );
}

// --- P11: `#[track_caller]` 必须一路传到用户的调用点 ---

/// 每个公开构造函数记录的 `defined_at` 都必须落在**本测试文件**里，
/// 而不是框架内部某一行。修复前它们指向 `runtime.rs` 的若干行，
/// 于是全工作区 7 处消费 `get_node_defined_at` 的调试信息基本上都是错的。
#[test]
fn defined_at_points_at_user_code_not_at_the_framework() {
    let first_line = line!();
    let cases: Vec<(&str, RawId)> = vec![
        ("signal", signal::create(0i32).raw()),
        ("effect", effect::create(|| {}).raw()),
        ("memo", memo::create(|_: Option<&i32>| 1i32).raw()),
        ("memo::derived", memo::derived(Box::new(|| 1i32)).raw()),
        ("store::create", store::create(1i32).raw()),
        ("store::create（装箱）", store::create(Box::new(1i32)).raw()),
        ("callback::create", callback::create(|_| {}).raw()),
        ("node_ref::create", node_ref::create::<i32>().raw()),
        ("scope::create", scope::create(|| {}).raw()),
    ];
    let last_line = line!();

    for (name, id) in cases {
        let Some(location) = get_node_defined_at(id) else {
            // release 构建下不记录定义位置。
            if cfg!(debug_assertions) {
                panic!("{name}: debug 构建必须记录定义位置");
            }
            continue;
        };

        assert!(
            location.file().ends_with("regression.rs"),
            "{name}: 定义位置指向了框架内部 {}:{}，应指向用户调用点",
            location.file(),
            location.line()
        );
        assert!(
            (first_line..last_line).contains(&location.line()),
            "{name}: 定义位置落在第 {} 行，不在上面那张表里",
            location.line()
        );
    }
}

// --- P12: 类型不匹配的写入不得产生任何失效 ---

#[test]
fn a_type_mismatched_update_changes_nothing_and_notifies_nobody() {
    let s = signal::create(1i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        runs_c.set(runs_c.get() + 1);
    });
    let before = runs.get();

    // `s` 里放的是 i32，这里按 String 写 —— 闭包不会执行。
    let outcome = signal::try_update(s, |v: &mut String| v.push('x'));

    assert_eq!(outcome, Err(ReactiveError::TypeMismatch));
    assert_eq!(signal::try_get::<i32>(s), Ok(1), "值不该被改动");
    assert_eq!(
        runs.get(),
        before,
        "什么都没改，下游不该被重跑（版本号也不该被递增）"
    );
}

/// 失败的写入不得沿着 memo 链把失效传下去。
#[test]
fn a_failed_update_does_not_invalidate_downstream_memos() {
    let s = signal::create(1i32);
    let recomputes = Rc::new(Cell::new(0));

    let recomputes_c = recomputes.clone();
    let m = memo::create(move |_: Option<&i32>| {
        recomputes_c.set(recomputes_c.get() + 1);
        signal::try_get::<i32>(s).unwrap_or(0) * 10
    });
    let after_first = recomputes.get();

    assert_eq!(
        signal::try_update(s, |v: &mut String| v.push('x')),
        Err(ReactiveError::TypeMismatch)
    );

    assert_eq!(signal::try_get::<i32>(m), Ok(10));
    assert_eq!(
        recomputes.get(),
        after_first,
        "失败的写入不该让下游 memo 重算"
    );
}

#[test]
fn update_outcomes_are_distinguishable() {
    let s = signal::create(1i32);
    assert_eq!(signal::try_update(s, |v: &mut i32| *v += 1), Ok(()));
    assert_eq!(signal::try_get::<i32>(s), Ok(2));

    // 节点不存在与类型不对是两回事。
    let (missing_owner, missing) = scope::create_detached(|| signal::create(0i32));
    scope::dispose(missing_owner);
    assert_eq!(
        signal::try_update(missing, |v: &mut i32| *v += 1),
        Err(ReactiveError::NoSuchNode)
    );
    assert_eq!(
        signal::try_update(s, |v: &mut u8| *v += 1),
        Err(ReactiveError::TypeMismatch)
    );

    // 在 update 闭包内重写同一个 signal 是被禁止的：
    // debug 构建下断言失败，release 下报告为 `Reentrant`。
    let inner: Rc<Cell<Option<ReactiveResult<()>>>> = Rc::new(Cell::new(None));
    let inner_c = inner.clone();
    let outer = silently(move || {
        signal::try_update(s, move |v: &mut i32| {
            *v += 1;
            inner_c.set(Some(signal::try_update(s, |w: &mut i32| *w += 100)));
        })
    });

    if cfg!(debug_assertions) {
        assert!(outer.is_err(), "debug 构建下重入必须触发断言");
    } else {
        assert_eq!(outer.ok(), Some(Ok(())));
        assert_eq!(inner.get(), Some(Err(ReactiveError::Reentrant)));
    }
}

// --- P18: 审查中实测“工作正常”的行为，固化下来防止回归 ---

/// 菱形依赖不得出现 glitch：effect 看到的两个上游值必须始终自洽，
/// 且一次更新只重跑一次。
#[test]
fn a_diamond_dependency_never_shows_an_intermediate_state() {
    let s = signal::create(1i32);
    let double = memo::create(move |_: Option<&i32>| signal::try_get::<i32>(s).unwrap_or(0) * 2);
    let triple = memo::create(move |_: Option<&i32>| signal::try_get::<i32>(s).unwrap_or(0) * 3);

    let seen: Rc<RefCell<Vec<(i32, i32)>>> = Rc::new(RefCell::new(Vec::new()));
    let seen_c = seen.clone();
    effect::create(move || {
        let d = signal::try_get::<i32>(double).unwrap_or(0);
        let t = signal::try_get::<i32>(triple).unwrap_or(0);
        seen_c.borrow_mut().push((d, t));
    });

    signal::update(s, |v: &mut i32| *v = 5);

    let seen = seen.borrow();
    assert_eq!(seen.len(), 2, "一次更新只该重跑一次，实际：{seen:?}");
    for &(d, t) in seen.iter() {
        assert_eq!(d * 3, t * 2, "两个上游必须来自同一个 s，实际：{seen:?}");
    }
    assert_eq!(seen.last(), Some(&(10, 15)));
}

/// 条件分支里不再读取的 signal 必须被退订。
#[test]
fn dependencies_are_re_collected_on_every_run() {
    let switch = signal::create(true);
    let a = signal::create(0i32);
    let b = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        runs_c.set(runs_c.get() + 1);
        if signal::try_get::<bool>(switch).unwrap_or(false) {
            let _ = signal::try_get::<i32>(a);
        } else {
            let _ = signal::try_get::<i32>(b);
        }
    });
    assert_eq!(runs.get(), 1);

    signal::update(b, |v: &mut i32| *v += 1);
    assert_eq!(runs.get(), 1, "当前分支没读 b，写 b 不该触发");

    signal::update(switch, |v: &mut bool| *v = false);
    assert_eq!(runs.get(), 2);

    signal::update(a, |v: &mut i32| *v += 1);
    assert_eq!(runs.get(), 2, "切换分支后必须已经退订 a");

    signal::update(b, |v: &mut i32| *v += 1);
    assert_eq!(runs.get(), 3, "新分支的依赖必须已经建立");
}

/// `untrack` 读到的值不建立依赖，但重跑时能看到最新值。
#[test]
fn untracked_reads_do_not_subscribe_but_still_see_fresh_values() {
    let tracked = signal::create(0i32);
    let hidden = signal::create(10i32);
    let seen = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));

    let seen_c = seen.clone();
    let runs_c = runs.clone();
    effect::create(move || {
        runs_c.set(runs_c.get() + 1);
        let _ = signal::try_get::<i32>(tracked);
        seen_c.set(scope::untrack(|| {
            signal::try_get::<i32>(hidden).unwrap_or(0)
        }));
    });
    assert_eq!((runs.get(), seen.get()), (1, 10));

    signal::update(hidden, |v: &mut i32| *v = 20);
    assert_eq!(runs.get(), 1, "untrack 读过的 signal 不该成为依赖");

    signal::update(tracked, |v: &mut i32| *v += 1);
    assert_eq!(
        (runs.get(), seen.get()),
        (2, 20),
        "重跑时 untrack 必须读到最新值"
    );
}

/// 销毁链条中间的 memo：下游会静默冻结在旧值上。这是当前的**既有行为**，
/// 写下来是为了让它变成一个决定，而不是一个意外。
#[test]
fn disposing_a_node_in_the_middle_freezes_its_downstream() {
    let s = signal::create(1i32);
    let (mid_owner, mid) = scope::create_detached(move || {
        memo::create(move |_: Option<&i32>| signal::try_get::<i32>(s).unwrap_or(0) * 10)
    });
    let tail = memo::create(move |_: Option<&i32>| signal::try_get::<i32>(mid).unwrap_or(-1));

    assert_eq!(signal::try_get::<i32>(tail), Ok(10));

    scope::dispose(mid_owner);
    signal::update(s, |v: &mut i32| *v = 2);

    assert!(!mid.is_alive());
    assert_eq!(
        signal::try_get::<i32>(tail),
        Ok(10),
        "上游被销毁后，下游冻结在最后一个已知值上"
    );
}

/// 销毁一个 effect 之后，它既不再运行，写它原来的依赖也不再有任何开销。
#[test]
fn a_disposed_effect_unsubscribes_from_everything() {
    let s = signal::create(0i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    let (effect_owner, _e) = scope::create_detached(move || {
        effect::create(move || {
            let _ = signal::try_get::<i32>(s);
            runs_c.set(runs_c.get() + 1);
        })
    });
    assert_eq!(runs.get(), 1);

    scope::dispose(effect_owner);
    signal::update(s, |v: &mut i32| *v += 1);
    assert_eq!(runs.get(), 1);
}

// --- P13: 环与自喂养队列必须是可诊断的报错，而不是挂死 ---

/// 两个互相依赖的 memo：修复前 `evaluate` 的 DFS 栈会一直增长到 OOM。
#[test]
fn a_dependency_cycle_panics_instead_of_growing_the_stack_forever() {
    let s = signal::create(0i32);
    // memo 只能在创建后才拿得到自己的 id，用一个格子把环接上。
    let second: Rc<Cell<Option<MemoId>>> = Rc::new(Cell::new(None));

    let second_c = second.clone();
    let first = memo::create(move |_: Option<&i32>| {
        let _ = signal::try_get::<i32>(s);
        match second_c.get() {
            Some(other) => signal::try_get::<i32>(other).unwrap_or(0),
            None => 0,
        }
    });

    let other = memo::create(move |_: Option<&i32>| signal::try_get::<i32>(first).unwrap_or(0) + 1);
    second.set(Some(other));

    // 第一次重算时 `first` 才会真的去读 `other`，环在这一步接上
    // （此时 `other` 的依赖 `first` 正在运行，会被跳过，所以还不会报错）。
    signal::update(s, |v: &mut i32| *v += 1);
    assert_eq!(signal::try_get::<i32>(first), Ok(1));

    // 环已经成型：再失效一次，求值 DFS 就会沿着 first -> other -> first 走回来。
    let result = silently(|| {
        signal::update(s, |v: &mut i32| *v += 1);
        signal::try_get::<i32>(first)
    });

    let err = result.expect_err("成环时必须 panic，而不是无限压栈");
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .unwrap_or_default();
    assert!(
        msg.contains("依赖环"),
        "报错信息里必须说明是依赖环，实际是：{msg}"
    );
    // 定义位置只在 debug 构建下记录（release 下 `defined_at` 恒为 None）。
    if cfg!(debug_assertions) {
        assert!(
            msg.contains("regression.rs"),
            "报错信息里必须带上环上节点的定义位置，实际是：{msg}"
        );
    }
}

/// 两个互相写对方依赖的 effect：修复前浏览器标签页直接冻死。
///
/// 这个用例要真的跑满十万次迭代才会触发上限，在 Miri 下慢得没有意义，
/// 而它考察的是调度逻辑、不涉及任何 `unsafe` 边界，因此 Miri 下跳过。
#[test]
#[cfg_attr(miri, ignore)]
fn a_self_feeding_effect_queue_panics_instead_of_hanging() {
    let a = signal::create(0i32);
    let b = signal::create(0i32);

    effect::create(move || {
        let _ = signal::try_get::<i32>(a);
        signal::update(b, |v: &mut i32| *v += 1);
    });

    let result = silently(move || {
        effect::create(move || {
            let _ = signal::try_get::<i32>(b);
            signal::update(a, |v: &mut i32| *v += 1);
        });
    });

    let err = result.expect_err("自我喂养的队列必须 panic，而不是挂死");
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .unwrap_or_default();
    assert!(
        msg.contains("effect 队列执行超过"),
        "报错信息必须指出是队列没有收敛，实际是：{msg}"
    );
}

/// P13 漏掉的另一半：**求值 DFS** 也会不收敛，而它一次都碰不到 effect 队列。
///
/// 一个节点只要在自己的运行过程中写回自己的上游，就会被立刻重新标脏，
/// 求值 DFS 于是原地把它重跑一遍，永不收敛。这条路径整个发生在
/// `drive_eval` 的循环里，`run_queue` 的迭代计数器一次都不会加 ——
/// 所以从前的表现是**真的挂死**（release 下跑满 90 秒仍不退出），
/// 而不是 P13 承诺的那句报错。
///
/// 这里用「被覆盖的旧值在析构里写回上游」来触发，因为它是纯安全代码，
/// 而且不需要用户显式写一个自我喂养的闭包 —— 只要 memo 的值类型带一个
/// 会碰响应式图的 `Drop` 就够了。
#[test]
#[cfg_attr(miri, ignore)]
fn a_self_feeding_evaluation_panics_instead_of_hanging() {
    use std::rc::Rc;

    let source = signal::create(0i32);

    #[derive(Clone)]
    struct WritesBack(Rc<Poke>);
    impl PartialEq for WritesBack {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }
    struct Poke(SignalId);
    impl Drop for Poke {
        fn drop(&mut self) {
            signal::update::<i32>(self.0, |v| *v += 1);
        }
    }

    let m = memo::create::<WritesBack, _>(move |_| {
        let _ = signal::get::<i32>(source);
        WritesBack(Rc::new(Poke(source)))
    });
    effect::create(move || {
        let _ = signal::get::<WritesBack>(m);
    });

    let result = silently(move || {
        signal::update::<i32>(source, |v| *v += 1);
    });

    let err = result.expect_err("不收敛的求值必须 panic，而不是挂死");
    let msg = err
        .downcast_ref::<String>()
        .map(String::as_str)
        .unwrap_or_default();
    assert!(
        msg.contains("单次求值执行了超过"),
        "报错信息必须指出是求值没有收敛，实际是：{msg}"
    );
}

// --- P10: 相等性门控策略必须是明确且稳定的 ---

/// signal 不做门控：写入相同的值照样重跑下游。这是**有意的**设计
/// （`signal::update` 交出 `&mut T`，运行时无从比较），固化在这里防止它
/// 在某次重构里被悄悄改掉。
#[test]
fn writing_an_equal_value_to_a_signal_still_notifies() {
    let s = signal::create(1i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        runs_c.set(runs_c.get() + 1);
    });
    assert_eq!(runs.get(), 1);

    signal::update(s, |v: &mut i32| *v = 1); // 值没变
    assert_eq!(runs.get(), 2, "signal 是无门控的");
}

/// 需要门控的调用方用 `signal::set_if_changed` 显式付费。
#[test]
fn set_signal_if_changed_gates_on_equality() {
    let s = signal::create(1i32);
    let runs = Rc::new(Cell::new(0));

    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        runs_c.set(runs_c.get() + 1);
    });
    assert_eq!(runs.get(), 1);

    assert_eq!(
        signal::set_if_changed(s, 1i32),
        Ok(false),
        "值相等：不写、不通知"
    );
    assert_eq!(runs.get(), 1, "值相等时下游不该动");

    assert_eq!(signal::set_if_changed(s, 2i32), Ok(true));
    assert_eq!(runs.get(), 2);
    assert_eq!(signal::try_get::<i32>(s), Ok(2));

    // 失败分支与 `signal::try_update` 保持一致。
    assert_eq!(
        signal::set_if_changed(s, String::new()),
        Err(ReactiveError::TypeMismatch)
    );
    let (gone_owner, gone) = scope::create_detached(|| signal::create(0i32));
    scope::dispose(gone_owner);
    assert_eq!(
        signal::set_if_changed(gone, 1i32),
        Err(ReactiveError::NoSuchNode)
    );
}

/// memo 做门控：上游变了但 memo 的值没变时，下游不该被重跑。
#[test]
fn a_memo_absorbs_updates_that_do_not_change_its_value() {
    let s = signal::create(1i32);
    let m = memo::create(move |_: Option<&i32>| signal::try_get::<i32>(s).unwrap_or(0) / 10);

    let runs = Rc::new(Cell::new(0));
    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(m);
        runs_c.set(runs_c.get() + 1);
    });
    assert_eq!(runs.get(), 1);

    signal::update(s, |v: &mut i32| *v = 2); // 2 / 10 仍是 0
    assert_eq!(runs.get(), 1, "memo 的值没变，下游不该被重跑");

    signal::update(s, |v: &mut i32| *v = 20); // 20 / 10 = 2
    assert_eq!(runs.get(), 2);
}

/// `memo::derived` 不做门控：它的 `T` 连 `PartialEq` 都没有，每次重算都
/// 通知下游。这条同样是**有意的**契约，写下来免得下次有人当成 bug “修掉”。
#[test]
fn a_derived_node_never_gates_on_equality() {
    let s = signal::create(1i32);
    let d = memo::derived(Box::new(move || {
        signal::try_get::<i32>(s).unwrap_or(0) / 10
    }));

    let runs = Rc::new(Cell::new(0));
    let runs_c = runs.clone();
    effect::create(move || {
        let _ = signal::try_get::<i32>(d);
        runs_c.set(runs_c.get() + 1);
    });
    assert_eq!(runs.get(), 1);

    signal::update(s, |v: &mut i32| *v = 2); // 派生值仍是 0，但下游照样重跑
    assert_eq!(runs.get(), 2, "derived 是无门控的");
}

// --- P9: memo 重算不得克隆旧值 ---

/// 记录自己被克隆过多少次。
#[derive(Debug)]
struct CloneCounted {
    v: i32,
    clones: Rc<Cell<usize>>,
}

impl Clone for CloneCounted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            v: self.v,
            clones: self.clones.clone(),
        }
    }
}

impl PartialEq for CloneCounted {
    fn eq(&self, other: &Self) -> bool {
        self.v == other.v
    }
}

/// 修复前：一次重算克隆旧值 **3 次**（运行时两次 + vtable 里 `cloned()` 一次），
/// 而且闭包用不用 `old` 都要付这个代价。现在旧值是借给闭包的。
#[test]
fn recomputing_a_memo_never_clones_the_old_value() {
    let clones = Rc::new(Cell::new(0));
    let s = signal::create(1i32);

    let clones_c = clones.clone();
    let m = memo::create(move |old: Option<&CloneCounted>| {
        let prev = old.map_or(0, |t| t.v);
        CloneCounted {
            v: prev + signal::try_get::<i32>(s).unwrap_or(0),
            clones: clones_c.clone(),
        }
    });

    // memo 是惰性的：每次写完都读一下，逼它真的重算。
    // 用 `signal::try_with` 读，避免读取本身产生的（正当的）克隆混进计数。
    signal::update(s, |v: &mut i32| *v = 10);
    assert_eq!(signal::try_with::<CloneCounted, _>(m, |t| t.v), Ok(11));
    signal::update(s, |v: &mut i32| *v = 100);
    assert_eq!(signal::try_with::<CloneCounted, _>(m, |t| t.v), Ok(111));
    assert_eq!(
        clones.get(),
        0,
        "旧值必须按引用传给计算闭包，运行时一次也不该克隆它"
    );
}

/// 闭包不使用 `old` 时同样不该有任何克隆开销。
#[test]
fn a_memo_that_ignores_its_old_value_pays_nothing_for_it() {
    let clones = Rc::new(Cell::new(0));
    let s = signal::create(1i32);

    let clones_c = clones.clone();
    let m = memo::create(move |_: Option<&CloneCounted>| CloneCounted {
        v: signal::try_get::<i32>(s).unwrap_or(0),
        clones: clones_c.clone(),
    });

    for i in 2..6 {
        signal::update(s, |v: &mut i32| *v = i);
    }

    assert_eq!(signal::try_with::<CloneCounted, _>(m, |t| t.v), Ok(5));
    assert_eq!(clones.get(), 0);
}

/// `signal::try_update_silent` 走的是另一条路径（silex_core 的写入入口），
/// 同样不该在类型不匹配时递增版本号。
///
/// 版本号是 `Check` 阶段判断“依赖变没变”的唯一依据，所以要观察它必须把 `m`
/// 逼进 `Check` 状态：由 `t` 触发一条**值不变**的 memo 链（`mid` 恒为 0，
/// 因此 `commit_update` 不会动它的版本号），`m` 随之被标成 `Check` 并逐个
/// 核对依赖版本。此时 `s` 的版本号只要被误增一次，`m` 就会白白重算。
#[test]
fn a_type_mismatched_silent_update_does_not_bump_the_version() {
    let s = signal::create(1i32);
    let t = signal::create(0i32);

    let mid = memo::create(move |_: Option<&i32>| {
        let _ = signal::try_get::<i32>(t);
        0i32
    });

    let recomputes = Rc::new(Cell::new(0));
    let recomputes_c = recomputes.clone();
    let m = memo::create(move |_: Option<&i32>| {
        recomputes_c.set(recomputes_c.get() + 1);
        let _ = signal::try_get::<i32>(mid);
        signal::try_get::<i32>(s).unwrap_or(0)
    });

    effect::create(move || {
        let _ = signal::try_get::<i32>(m);
    });
    let before = recomputes.get();

    assert_eq!(
        signal::try_update_silent(s, |v: &mut String| v.push('x')),
        Err(ReactiveError::TypeMismatch)
    );
    assert_eq!(signal::try_get::<i32>(s), Ok(1), "值不该被改动");

    // 从另一条边把 m 标成 Check。
    signal::update(t, |v: &mut i32| *v += 1);

    assert_eq!(
        recomputes.get(),
        before,
        "`mid` 的值没变、`s` 的版本号也不该变，`m` 不该重算"
    );
}
