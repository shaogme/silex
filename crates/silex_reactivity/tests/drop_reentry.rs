//! 用户的 `Drop` 回头访问响应式图。
//!
//! 值离开图的地方（节点销毁、memo 提交覆盖旧值）从前是**就地析构**：
//! 用户的 `Drop` 因此运行在运行时对该节点的借用之内。方案 B 把访问入口收成
//! `&mut Runtime` 之后每一处都会变成借用冲突，所以这些位置改成把值推进
//! “墓园”，由驱动循环在释放借用之后统一析构（设计文档 §4.2）。
//!
//! 这个文件钉两件事：
//!
//! 1. `Drop` 里访问运行时是**允许**的，而且真的生效；
//! 2. 析构发生的**时机**没有变 —— 墓园的排空点选在与从前就地析构相同的位置上。

use silex_reactivity::*;
use std::{cell::RefCell, rc::Rc};

type Log = Rc<RefCell<Vec<String>>>;

fn log() -> Log {
    Rc::new(RefCell::new(Vec::new()))
}

fn taken(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

/// 析构时往日志里记一笔，并**回头访问运行时**。
struct Spy {
    tag: &'static str,
    log: Log,
    /// 析构时要递增的计数器 signal。
    poke: Option<SignalId>,
    /// 同上，但目标存的是 `i32`（用于写回上游的那条用例）。
    poke_i32: Option<SignalId>,
}

impl Drop for Spy {
    fn drop(&mut self) {
        self.log.borrow_mut().push(format!("drop-{}", self.tag));
        // 这两行就是本文件的全部意义：用户的 `Drop` 在改响应式图。
        if let Some(s) = self.poke {
            signal::update::<usize>(s, |v| *v += 1);
        }
        if let Some(s) = self.poke_i32 {
            signal::update::<i32>(s, |v| *v += 1);
        }
    }
}

fn spy(tag: &'static str, log: &Log, poke: Option<SignalId>) -> Spy {
    Spy {
        tag,
        log: log.clone(),
        poke,
        poke_i32: None,
    }
}

/// 销毁一个 scope 时，子节点载荷的 `Drop` 可以写 signal，而且写入真的生效。
#[test]
fn a_payload_destructor_may_write_a_signal_while_its_scope_is_disposed() {
    let log = log();
    let counter = signal::create(0usize);

    let root = scope::create({
        let log = log.clone();
        move || {
            store::create(spy("stored", &log, Some(counter)));
        }
    });

    assert_eq!(signal::get::<usize>(counter), Some(0));
    scope::dispose(root);

    assert_eq!(taken(&log), vec!["drop-stored"]);
    assert_eq!(
        signal::get::<usize>(counter),
        Some(1),
        "析构里的写入必须真的落到 signal 上"
    );
}

/// effect 的闭包捕获的值，在 effect 被销毁时同样可以在 `Drop` 里访问运行时。
#[test]
fn an_effect_closure_destructor_may_read_the_graph() {
    let log = log();
    let counter = signal::create(0usize);
    let source = signal::create(1i32);

    let e = effect::create({
        let held = spy("captured", &log, Some(counter));
        move || {
            // 让闭包真的捕获 `held`（否则它会被优化掉/提前析构）。
            let _ = &held;
            let _ = signal::get::<i32>(source);
        }
    });

    assert!(taken(&log).is_empty(), "effect 还活着，闭包不该被析构");
    scope::dispose(e);

    assert_eq!(taken(&log), vec!["drop-captured"]);
    assert_eq!(signal::get::<usize>(counter), Some(1));
}

/// memo 重算覆盖旧值时，旧值的 `Drop` 可以访问运行时。
///
/// 这条走的是 `commit_update` 那个覆盖点，与销毁路径是两套代码。
#[test]
fn a_replaced_memo_value_may_touch_the_runtime_while_being_dropped() {
    let log = log();
    let counter = signal::create(0usize);
    let source = signal::create(0i32);

    // memo 的值带析构函数：每次重算，上一轮的值都会被覆盖掉。
    #[derive(Clone)]
    struct Boxed(Rc<Spy>);
    impl PartialEq for Boxed {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }

    let l = log.clone();
    let m = memo::create::<Boxed, _>(move |_| {
        let n = signal::get::<i32>(source).unwrap_or(0);
        // 每轮造一个新的（`ptr_eq` 因此恒不相等，一定会覆盖）。
        let _ = n;
        Boxed(Rc::new(spy("memo", &l, Some(counter))))
    });

    let _ = signal::get::<Boxed>(m); // 触发首算
    assert!(taken(&log).is_empty(), "首算没有旧值可覆盖");

    signal::update::<i32>(source, |v| *v += 1);
    let _ = signal::get::<Boxed>(m); // 触发重算 -> 覆盖旧值

    assert_eq!(taken(&log), vec!["drop-memo"], "旧值必须被析构掉");
    assert_eq!(
        signal::get::<usize>(counter),
        Some(1),
        "旧值析构里的写入必须生效"
    );
}

/// 旧值的 `Drop` 写回这个 memo **自己依赖的** signal。
///
/// 这是墓园唯一修掉的真实缺陷。从前 `commit_update` 是这么写的：
///
/// ```ignore
/// node.signal.borrow_mut().value = Some(value);   // 赋值就地析构掉旧值
/// ```
///
/// `RefMut` 活到整条语句结束，于是旧值的 `Drop` 跑在这个 `borrow_mut` 之内。
/// 它往 `source` 写一笔 → `propagate` 从 `source` 出发 → 订阅者里就有本 memo →
/// 它不是 effect，于是被推进 BFS 队列 → 弹出来遍历**它自己的**订阅者表 →
/// `node.signal.borrow()` 撞上还没释放的 `borrow_mut` → panic。
///
/// 换句话说：只要一个 memo 的值带析构函数、而析构函数碰了这个 memo 的上游，
/// 整个运行时就炸。偏门，但完全是安全代码写得出来的。
#[test]
fn a_replaced_value_may_write_back_to_its_own_upstream() {
    let log = log();
    let source = signal::create(0i32);

    #[derive(Clone)]
    struct WritesBack(Rc<Spy>);
    impl PartialEq for WritesBack {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }

    // 只回写一次。无条件回写会构成一个真正的无限反馈环
    //（写上游 → 重算 → 覆盖旧值 → 再写上游），那考察的是别的东西。
    let armed = Rc::new(std::cell::Cell::new(false));

    let l = log.clone();
    let a = armed.clone();
    let m = memo::create::<WritesBack, _>(move |_| {
        let _ = signal::get::<i32>(source); // 建立对 source 的依赖
        WritesBack(Rc::new(Spy {
            tag: "old",
            log: l.clone(),
            // 析构时写回自己的上游 —— 就是这一笔把 propagate 引回本节点。
            poke_i32: a.replace(false).then_some(source),
            poke: None,
        }))
    });
    // 下游得有个订阅者，propagate 才会真的去遍历 memo 的订阅者表。
    effect::create(move || {
        let _ = signal::get::<WritesBack>(m);
    });

    // 武装：接下来这一轮算出来的值，析构时会写回 source。
    armed.set(true);
    signal::update::<i32>(source, |v| *v += 1);
    let _ = signal::get::<WritesBack>(m);

    // 再触发一轮，把那个「武装过的」值覆盖掉 —— 它的析构就在这里发生。
    signal::update::<i32>(source, |v| *v += 1);
    let _ = signal::get::<WritesBack>(m);

    assert!(
        log.borrow().iter().any(|s| s == "drop-old"),
        "旧值必须被析构掉"
    );
}

/// 析构的**时机**：cleanup 先于本节点的载荷析构，子树先于父节点。
///
/// 这条是墓园设计的验收标准 —— 排空点必须选在与从前就地析构相同的位置上，
/// 否则所有载荷的 `Drop` 会被推迟到整棵子树销毁完毕，用户可观察的顺序就变了。
#[test]
fn destruction_order_is_unchanged_by_the_graveyard() {
    let log = log();

    let root = scope::create({
        let log = log.clone();
        move || {
            for tag in ["a", "b"] {
                let log = log.clone();
                scope::create(move || {
                    store::create(spy(tag, &log, None));
                    let l = log.clone();
                    scope::on_cleanup(move || l.borrow_mut().push(format!("cleanup-{tag}")));
                });
            }
            let l = log.clone();
            scope::on_cleanup(move || l.borrow_mut().push("cleanup-root".into()));
        }
    });

    scope::dispose(root);

    assert_eq!(
        taken(&log),
        vec!["drop-a", "cleanup-a", "drop-b", "cleanup-b", "cleanup-root",],
        "子节点的载荷先析构，然后才是所属 scope 的 cleanup；同级按注册顺序"
    );
}

/// `Drop` 里销毁**别的**节点是允许的：墓园的排空是可重入的。
#[test]
fn a_destructor_may_dispose_another_node() {
    let log = log();

    let victim = store::create(spy("victim", &log, None));

    let root = scope::create({
        let log = log.clone();
        move || {
            let l = log.clone();
            // 这个 stored value 的析构会把 `victim` 一起销毁。
            store::create(Killer {
                tag: "killer",
                log: l,
                victim,
            });
        }
    });

    struct Killer {
        tag: &'static str,
        log: Log,
        victim: StoredId,
    }
    impl Drop for Killer {
        fn drop(&mut self) {
            self.log.borrow_mut().push(format!("drop-{}", self.tag));
            scope::dispose(self.victim);
        }
    }

    scope::dispose(root);

    assert_eq!(taken(&log), vec!["drop-killer", "drop-victim"]);
    assert!(!victim.is_alive(), "被牵连的节点确实销毁了");
}
