//! 阶段二（破坏性 API 重塑）的行为固化。
//!
//! 覆盖四件事：
//!
//! 1. **载荷访问的重入语义**（§2.1）：所有交给用户闭包的访问都会先把值移出节点，
//!    因此在闭包内访问同一个节点会拿到 `Reentrant`，而不是静默的别名违规；
//! 2. **失败原因可区分**（§3.2）：`NoRuntime` / `NoSuchNode` / `WrongKind` /
//!    `TypeMismatch` / `Reentrant` 五种，而不是压成一个 `None`；
//! 3. **载荷会被正确析构**（§2.4）：`RawOpBuffer` 那条永不析构的路径已经删除；
//! 4. **`effect` 接受 `FnMut`**（§3.4）。
//!
//! 种类安全（§3.1）本身是**编译期**性质，没法用运行时断言表达 ——
//! `signal::try_get::<i32>(stored_id)` 现在压根编译不过。这里只固化那部分
//! 仍然发生在运行时的：用 `RawId` 显式擦除之后的 `WrongKind`。

use silex_reactivity::*;
use std::{cell::Cell, rc::Rc};

// --- 1. 重入语义 ---

/// 在 `store::try_with` 的闭包里读同一个节点 —— 值此刻在闭包手里，节点里是占位值。
#[test]
fn borrowing_a_stored_value_twice_reports_reentrancy() {
    let sv = store::create(1i32);

    let inner = store::try_with::<i32, _>(sv, |_| store::try_with::<i32, _>(sv, |v| *v))
        .expect("外层借用应当成功");

    assert_eq!(inner, Err(ReactiveError::Reentrant));
    // 出了闭包立刻恢复正常。
    assert_eq!(store::try_with::<i32, _>(sv, |v| *v), Ok(1));
}

/// 写入侧同理，而且值不会因为内层失败而丢掉。
#[test]
fn updating_a_stored_value_twice_reports_reentrancy() {
    let sv = store::create(1i32);

    let inner = store::try_update::<i32, _>(sv, |v| {
        *v = 7;
        store::try_update::<i32, _>(sv, |w| *w = 99)
    })
    .expect("外层更新应当成功");

    assert_eq!(inner, Err(ReactiveError::Reentrant));
    assert_eq!(
        store::try_with::<i32, _>(sv, |v| *v),
        Ok(7),
        "外层的写入必须生效"
    );
}

/// signal 的只读借用也走“移出—闭包—放回”，理由见 §2.1 违反 #3：
/// 闭包是用户代码，它可以写任何别的节点，那一次写入会作废运行时手里的引用。
#[test]
fn reading_a_signal_inside_its_own_with_closure_reports_reentrancy() {
    let s = signal::create(5i32);

    let inner =
        signal::try_with::<i32, _>(s, |_| signal::try_get::<i32>(s)).expect("外层借用应当成功");

    assert_eq!(inner, Err(ReactiveError::Reentrant));
    assert_eq!(signal::try_get::<i32>(s), Ok(5));
}

/// 而回调**允许**重入：`invoke` 先把 `Rc` 克隆出来、归还载荷，然后才调用。
#[test]
fn a_callback_may_invoke_itself() {
    let depth = Rc::new(Cell::new(0usize));
    let slot: Rc<Cell<Option<CallbackId>>> = Rc::new(Cell::new(None));

    let d = depth.clone();
    let s = slot.clone();
    let cb = callback::create(move |_arg| {
        d.set(d.get() + 1);
        if d.get() < 3 {
            let me = s.get().expect("句柄已经填好");
            callback::invoke(me, Box::new(())).expect("重入调用应当成功");
        }
    });
    slot.set(Some(cb));

    callback::invoke(cb, Box::new(())).expect("首次调用");
    assert_eq!(depth.get(), 3);
}

// --- 2. 失败原因可区分 ---

#[test]
fn typed_handles_round_trip_through_erasure() {
    let signal_id = signal::create(1i32);
    let stored_id = store::create(2i32);

    assert_eq!(SignalId::from_raw_unchecked(signal_id.raw()), signal_id);
    assert_eq!(StoredId::from_raw_unchecked(stored_id.raw()), stored_id);
}

#[test]
fn every_failure_reason_is_distinguishable() {
    let s = signal::create(1i32);
    let sv = store::create(1i32);

    // 类型不对：节点在、种类对，只是里面不是 String。
    assert_eq!(
        signal::try_get::<String>(s),
        Err(ReactiveError::TypeMismatch)
    );

    // 种类不对：用擦除句柄把一个 stored value 当 signal 读。
    // （带种类的句柄根本编译不过，这是逃生出口上才会出现的失败。）
    assert_eq!(
        signal::try_get::<i32>(sv.raw()),
        Err(ReactiveError::WrongKind)
    );

    // 节点不存在。
    let (gone_owner, gone) = scope::create_detached(|| signal::create(0i32));
    scope::dispose(gone_owner);
    assert_eq!(signal::try_get::<i32>(gone), Err(ReactiveError::NoSuchNode));

    // effect 没有值可读，同样是种类不对而不是“查无此节点”。
    let e = effect::create(|| {});
    assert_eq!(
        signal::try_get::<i32>(e.raw()),
        Err(ReactiveError::WrongKind)
    );
}

/// 便捷形式把所有失败折叠成 `None` —— 保留它是为了“节点没了就当没有值”
/// 确实正确的那些调用点。
#[test]
fn the_convenience_form_folds_every_failure_into_none() {
    let (gone_owner, gone) = scope::create_detached(|| signal::create(0i32));
    scope::dispose(gone_owner);

    assert_eq!(signal::get::<i32>(gone), None);
    assert_eq!(signal::try_get::<i32>(gone), Err(ReactiveError::NoSuchNode));
}

// --- 3. 载荷会被正确析构 ---

/// `Rx::new_op` 从前把载荷塞进一个 `[MaybeUninit<u8>; 64] + Copy` 的
/// `RawOpBuffer`，节点销毁时只丢掉 64 字节原始内存，**载荷的析构函数永远不会
/// 运行**（§2.4）。改用 `store::create` 之后带析构的载荷也是安全的。
#[test]
fn a_payload_with_a_destructor_is_actually_dropped() {
    struct Spy(Rc<Cell<usize>>);
    impl Drop for Spy {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let hits = Rc::new(Cell::new(0));

    let scope = scope::create(|| {
        // 小载荷（走 `AnyValue` 的内联表示）与大载荷（走 `Box`）各一个。
        store::create(Spy(hits.clone()));
        store::create((Spy(hits.clone()), [0u64; 16]));
    });
    assert_eq!(hits.get(), 0);

    scope::dispose(scope);
    assert_eq!(hits.get(), 2, "载荷的析构函数必须随节点销毁一起运行");
}

/// node-ref 就是一个存 `Option<T>` 的保管节点，里面的元素同样要被析构。
#[test]
fn a_node_ref_round_trips_and_is_disposed() {
    let (nr_owner, nr) = scope::create_detached(node_ref::create::<String>);
    assert_eq!(node_ref::try_get::<String>(nr), Ok(None), "尚未填充");

    node_ref::set(nr, "hello".to_string()).expect("填充");
    assert_eq!(node_ref::get::<String>(nr).as_deref(), Some("hello"));

    // 类型不符是一个明确的失败，不是 `None`。
    assert_eq!(
        node_ref::try_get::<i32>(nr),
        Err(ReactiveError::TypeMismatch)
    );

    scope::dispose(nr_owner);
    assert!(!nr.is_alive());
    assert_eq!(
        node_ref::try_get::<String>(nr),
        Err(ReactiveError::NoSuchNode)
    );
}

// --- 4. `effect` 接受 `FnMut` ---

/// 从前必须自己套一层 `Cell` / `RefCell` 才能在 effect 里维护状态（§3.4）。
#[test]
fn an_effect_may_capture_mutable_state() {
    let s = signal::create(0i32);
    let seen = Rc::new(Cell::new(0i32));

    let out = seen.clone();
    // `runs` 直接被 `move` 进闭包，不需要任何内部可变性包装。
    let mut runs = 0i32;
    effect::create(move || {
        let _ = signal::try_get::<i32>(s);
        runs += 1;
        out.set(runs);
    });

    assert_eq!(seen.get(), 1);
    signal::update::<i32>(s, |v| *v += 1);
    assert_eq!(seen.get(), 2);
    signal::update::<i32>(s, |v| *v += 1);
    assert_eq!(seen.get(), 3, "闭包捕获的可变状态必须跨重跑保留");
}

// --- 句柄本身 ---

/// 句柄的相等性只看它指向哪个节点；不同种类的句柄类型互不通约，
/// 所以“把 memo 的 id 当 signal 的 id 比”这种事在类型层面就不存在了。
#[test]
fn handles_compare_by_identity_and_survive_disposal() {
    let (a_owner, a) = scope::create_detached(|| signal::create(1i32));
    let b = signal::create(1i32);

    assert_eq!(a, a);
    assert_ne!(a, b);
    assert!(a.is_alive());

    scope::dispose(a_owner);
    assert!(!a.is_alive());
    // 句柄是 `Copy` 的，销毁之后它仍然可以传递，只是不再指向任何东西。
    assert_eq!(a, a);
    assert_ne!(a, b);
}

/// 空句柄对每一种查询都是“不存在”，而且不触发任何分配
/// （伪造的巨大 index 曾经会让二级表 `resize_with` 出巨量内存，§3.4）。
#[test]
fn the_dangling_handle_is_inert() {
    // 先建一个真节点，把本线程的运行时建起来 —— 否则每一条查询都会先撞上
    // `NoRuntime`（那本身也是对的：没有运行时就不可能有节点，AUDIT P19.9）。
    let _ = signal::create(0u8);

    assert!(!SignalId::DANGLING.is_alive());
    assert_eq!(
        signal::try_get::<i32>(SignalId::DANGLING),
        Err(ReactiveError::NoSuchNode)
    );
    assert_eq!(
        store::try_with::<i32, _>(StoredId::DANGLING, |v| *v),
        Err(ReactiveError::NoSuchNode)
    );
    // 销毁一个空句柄是 no-op，不 panic。
    scope::dispose(ScopeId::DANGLING);
}
