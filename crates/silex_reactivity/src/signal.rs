//! 图的根：存一个值，每一次成功的写入都通知下游。
//!
//! 读取相关的函数接受任何 [`Readable`] 句柄（signal / memo / derived），
//! 写入相关的只接受 [`SignalId`] —— 派生值不能被直接写，这条从前是运行时的
//! 一个静默 `None`，现在是编译错误。

use crate::{
    ReactiveError, ReactiveResult, Readable, SignalId,
    handle::RawId,
    internal::value::AnyValue,
    runtime::{drive, with_rt},
};

/// 创建一个 signal（响应式图的根）。
///
/// # 相等性门控策略
///
/// 整个 crate 只有一条规则，这里写死它（AUDIT P10）：
///
/// | 节点 | 何时通知下游 | 原因 |
/// |---|---|---|
/// | [`signal::create`](create) | **每一次成功的写入** | 值只有 `T: 'static`，没有 `PartialEq` 可用 |
/// | [`memo::create`](crate::memo::create) | 仅当新值 `!=` 旧值 | 签名要求 `T: Clone + PartialEq`，能比较 |
/// | [`memo::derived`](crate::memo::derived) | **每一次重算** | 值只有 `T: 'static`，无法比较 |
///
/// 也就是说 signal 是“无门控”的：写入相同的值同样会把全部下游重跑一遍。
/// 需要门控请用 [`set_if_changed`]（要求 `T: PartialEq`），或者把下游包一层
/// [`memo`](crate::memo::create) —— memo 会把重复的值挡在自己这一层。
#[track_caller]
pub fn create<T: 'static>(value: T) -> SignalId {
    SignalId::from_raw(drive::create_signal(AnyValue::new(value)).expect("刚建出来的运行时可用"))
}

// --- 读 ---

/// 读取当前值并**追踪依赖**。
///
/// 在 effect 或 memo 的计算闭包里调用会把该节点登记为当前节点的依赖。
/// 读取一个 memo 可能会驱动它的重算（惰性求值），因此这次调用可能同步执行
/// 用户代码。
///
/// # 契约
///
/// `T::clone` 在运行时的独占借用之内执行。`clone` 里对运行时的调用会拿不到
/// 借用并返回 [`Reentrant`](ReactiveError::Reentrant)。需要在读的时候执行任意
/// 用户代码请用 [`try_with`]，那条路径会先把值移出节点。
pub fn try_get<T: Clone + 'static>(id: impl Readable) -> ReactiveResult<T> {
    read(id.into_raw(), true)
}

/// 同 [`try_get`]，但**不**登记依赖。
pub fn try_get_untracked<T: Clone + 'static>(id: impl Readable) -> ReactiveResult<T> {
    read::<T>(id.into_raw(), false)
}

/// 读取的公共实现。
///
/// 快路径（节点已经干净、队列里也没有待办 —— 普通 signal 恒是这样）在
/// **一次借用**里把求值判定、依赖追踪、取值、克隆全做完，因此读一次 signal
/// 仍然只付一次线程本地查表 + 一次借用计数。
fn read<T: Clone + 'static>(raw: RawId, track: bool) -> ReactiveResult<T> {
    let fast = with_rt(|rt| {
        if !rt.is_settled(raw) {
            return None;
        }
        if track {
            rt.track_dependency(raw);
        }
        Some(rt.signal_value(raw).and_then(|v| downcast_cloned::<T>(v)))
    })?;
    if let Some(result) = fast {
        return result;
    }

    // 慢路径：要驱动求值，那可能同步执行用户代码，因此必须在借用之外。
    if track {
        drive::prepare_read(raw);
    } else {
        drive::prepare_read_untracked(raw);
    }
    with_rt(|rt| rt.signal_value(raw).and_then(|v| downcast_cloned::<T>(v)))?
}

/// [`try_get`] 的便捷形式：把任何失败折叠成 `None`。
///
/// 用在“节点没了就当没有值”确实是正确处理的地方。需要区分失败原因（尤其是
/// [`TypeMismatch`](ReactiveError::TypeMismatch) 这种编程错误）请用 [`try_get`]。
#[inline(always)]
pub fn get<T: Clone + 'static>(id: impl Readable) -> Option<T> {
    try_get(id).ok()
}

/// 借用当前值（追踪依赖），省掉 [`try_get`] 的那次克隆。
///
/// # 契约
///
/// 值在 `f` 执行期间被**移出**节点（节点里暂时是空的），运行时因此不必在用户
/// 代码执行期间持有任何指向该节点的引用（审计报告 §2.1）。代价是：
/// **不允许在 `f` 内部访问同一个节点**，那样会拿到
/// [`Reentrant`](ReactiveError::Reentrant)。
pub fn try_with<T: 'static, R>(id: impl Readable, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
    let raw = id.into_raw();
    drive::prepare_read(raw);
    with_typed(raw, f)
}

/// 同 [`try_with`]，但**不**登记依赖。
pub fn try_with_untracked<T: 'static, R>(
    id: impl Readable,
    f: impl FnOnce(&T) -> R,
) -> ReactiveResult<R> {
    let raw = id.into_raw();
    drive::prepare_read_untracked(raw);
    with_typed(raw, f)
}

// --- 写 ---

/// 就地修改一个 signal 的值并失效下游。
///
/// 写入失败时什么都不会发生 —— 既不改值，也不递增版本号，也不触发下游。
///
/// # 相等性
///
/// **成功的写入一律触发下游，不做任何相等性比较**（AUDIT P10）—— 哪怕 `f`
/// 什么都没改。这是有意的：`f` 拿到的是 `&mut T`，运行时无从得知它改了什么，
/// 而为了比较去克隆一份旧值，代价要由所有写入承担。需要“值不变就不通知”请用
/// [`set_if_changed`]。
///
/// # 契约
///
/// 不允许在 `f` 内部访问同一个 signal（读或写都不行）：值在 `f` 执行期间被移出
/// 了节点，此时节点里是空的。
pub fn try_update<T: 'static>(id: SignalId, f: impl FnOnce(&mut T)) -> ReactiveResult<()> {
    let mut f = Some(f);
    let mut mismatch = false;
    let applied = drive::update_signal_untyped(id.raw(), &mut |any_val| {
        let Some(val) = any_val.downcast_mut::<T>() else {
            mismatch = true;
            return false;
        };
        match f.take() {
            Some(f) => {
                f(val);
                true
            }
            // updater 至多被调用一次，走不到这里。
            None => false,
        }
    })?;

    if applied {
        Ok(())
    } else {
        debug_assert!(mismatch, "更新未生效，但类型是对的：这是运行时的 bug");
        Err(ReactiveError::TypeMismatch)
    }
}

/// [`try_update`] 的便捷形式：失败时静默丢弃。
///
/// 编程错误（种类不对 / 类型不符 / 重入，见
/// [`ReactiveError::is_bug`]）在 debug 构建下断言失败；
/// “节点已销毁”这种正常的运行时状态则一律安静跳过。
#[inline(always)]
pub fn update<T: 'static>(id: SignalId, f: impl FnOnce(&mut T)) {
    if let Err(e) = try_update::<T>(id, f) {
        debug_assert!(
            !e.is_bug(),
            "signal::update 失败（{e}）：节点 {id:?}，请求的类型是 {}",
            std::any::type_name::<T>()
        );
    }
}

/// 只在新值与当前值**不相等**时才写入并失效下游。
///
/// signal 本身不做相等性门控（见 [`create`]）。需要门控的调用方在这里显式付出
/// 一次 `PartialEq::eq` 的代价，而不是让所有写入都为此付费。
///
/// 返回值是“这次写入有没有真的发生”：`Ok(false)` 表示新值与当前值相等，
/// 值不变、版本号不变、下游不动。
pub fn set_if_changed<T: PartialEq + 'static>(id: SignalId, value: T) -> ReactiveResult<bool> {
    let mut incoming = Some(value);
    let mut equal = false;
    let applied = drive::update_signal_untyped(id.raw(), &mut |any_val| {
        let Some(slot) = any_val.downcast_mut::<T>() else {
            return false;
        };
        // updater 至多被调用一次，`take` 必定拿得到值。
        let Some(new_value) = incoming.take() else {
            return false;
        };
        if *slot == new_value {
            equal = true;
            return false;
        }
        *slot = new_value;
        true
    })?;

    match (applied, equal) {
        (true, _) => Ok(true),
        (false, true) => Ok(false),
        (false, false) => Err(ReactiveError::TypeMismatch),
    }
}

/// 就地修改一个 signal 的值，但**不**调度下游 —— 调用方需要自己在合适的时机
/// 调用 [`notify`]。版本号照常递增，否则下游的 `Check` 会误判“依赖没变”。
///
/// 类型不匹配时值不变，此时版本号也不会被递增（AUDIT P12）。
pub fn try_update_silent<T: 'static, R>(
    id: SignalId,
    f: impl FnOnce(&mut T) -> R,
) -> ReactiveResult<R> {
    let mut out = None;
    // 第二个分量即“请递增版本号”：只有类型相符、闭包真的跑过才递增。
    drive::with_signal_value_mut(id.raw(), |value| match value.downcast_mut::<T>() {
        Some(typed) => {
            out = Some(f(typed));
            ((), true)
        }
        None => ((), false),
    })?;
    out.ok_or(ReactiveError::TypeMismatch)
}

/// 手工失效一个 signal 的下游，不改动它的值。
///
/// 配合 [`try_update_silent`]：先静默写入若干次，最后统一通知一次。
pub fn notify(id: SignalId) {
    drive::notify_update(id.raw());
}

// --- 依赖追踪 ---

/// 把 `id` 登记为当前运行中节点的依赖，但不读取它的值。
///
/// 用于“只关心变化、不关心值”的场景。当前没有正在运行的节点时什么都不做。
pub fn track(id: impl Readable) {
    let _ = with_rt(|rt| rt.track_dependency(id.into_raw()));
}

/// [`track`] 的批量版本，只走一遍当前节点的查找。
///
/// 取 [`RawId`] 而不是带种类的句柄：调用方（上层框架的类型擦除分发）
/// 手里本来就是一个混种类的 id 数组。不是 signal 的条目会被静默跳过。
pub fn track_batch(ids: &[RawId]) {
    let _ = with_rt(|rt| rt.track_dependencies(ids));
}

// --- 逃生出口 ---

/// 直接借出 signal 当前值的引用（不追踪依赖），跳过 `Clone`。
///
/// # Safety
///
/// 返回的 `&'static T` 是**伪造**的：数据实际存活在运行时的 arena 里，与
/// `'static` 毫无关系。调用方必须保证在该引用的整个使用期间，下列操作一次都
/// 不发生 —— 任何一条都会让引用悬垂，读它即为未定义行为（AUDIT P6）：
///
/// - [`dispose`](crate::scope::dispose) 该节点或它的任一祖先（释放 arena 槽位）；
/// - [`try_update`] / [`try_update_silent`] / [`try_with`]：它们会把值**移出**
///   节点交给用户闭包，期间节点里是空的；
/// - memo 重算后的提交会整体替换掉这个值；
/// - 值从内联存储升级到堆上（`AnyValue` 的 SOO）；
/// - **任何会重入运行时并执行用户代码的调用** —— effect 体、cleanup、
///   `batch` 收尾、乃至读一个 memo（会驱动惰性求值）。
///
/// 本函数自身还会驱动一次惰性求值，这条路径上可能同步执行下游 effect ——
/// 也就是说**在返回之前**就已经跑过用户代码了。拿到引用之后请立刻用掉。
pub unsafe fn try_value_ref<T: 'static>(id: impl Readable) -> Option<&'static T> {
    let raw = id.into_raw();
    drive::prepare_read_untracked(raw);
    // SAFETY: 契约（引用不得跨越上面列出的任何一种操作）由本函数的调用方承担，
    // 原样转嫁给 `signal_value_unchecked`。指针要走出 `with_rt` 的借用再解引用，
    // 而“伪造出来的 `'static` 有多久有效”正是上面那份契约的内容。
    let ptr = with_rt(|rt| unsafe { rt.signal_value_unchecked(raw) }.map(std::ptr::from_ref))
        .ok()
        .flatten()?;
    unsafe { &*ptr }.downcast_ref::<T>()
}

// --- 内部辅助 ---

/// 读路径的 downcast：成功就克隆一份。
///
/// 这里从前还要调一个 `#[cold]` 的 `classify_value_failure` 去分辨“值正被借出”
/// 与“类型写错了”—— 因为借出期间节点里放的是一个占位 `AnyValue`，两种失败在
/// downcast 层面长得一模一样。阶段三之后借出就是一个 `None`，
/// [`Runtime::signal_value`] 直接报 [`ReactiveError::Reentrant`]，
/// 走到这里的失败只剩类型不符一种。
#[inline]
fn downcast_cloned<T: Clone + 'static>(value: &AnyValue) -> ReactiveResult<T> {
    value
        .downcast_ref::<T>()
        .cloned()
        .ok_or(ReactiveError::TypeMismatch)
}

/// 把值移出节点、downcast、交给用户闭包、放回。
///
/// 类型不符时也要先移出再放回：判定类型需要看一眼值，而“看一眼”本身就是那个
/// 不能跨越用户代码的借用。这样写的好处是失败路径与成功路径共用同一条纪律。
fn with_typed<T: 'static, R>(raw: RawId, f: impl FnOnce(&T) -> R) -> ReactiveResult<R> {
    drive::with_signal_value(raw, |value| {
        value
            .downcast_ref::<T>()
            .map(f)
            .ok_or(ReactiveError::TypeMismatch)
    })?
}
