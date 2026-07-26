use crate::{
    RawOpBuffer,
    core::{
        FuncPtr,
        arena::Index as NodeId,
        value::{AnyValue, ThunkValue},
    },
    runtime::{MemoVTable, RUNTIME, Runtime, SignalBorrowError, storage::ExtraData},
};
use silex_vtable::InlineStorage;
use std::{any::Any, marker::PhantomData, mem::size_of, ptr::drop_in_place, rc::Rc};

/// memo 内联载荷的布局：`[&'static MemoVTable][F 或 Box<F>]`。
///
/// vtable 以真正的指针形式写入缓冲区，**不做** `as usize` 往返 ——
/// 整数往返会擦除 provenance，之后再解引用即为未定义行为（AUDIT P3）。
const MEMO_PAYLOAD_OFFSET: usize = size_of::<usize>();

/// 把 vtable 指针与闭包打包进内联缓冲区。
/// 闭包放不下时改为存放 `Box<F>`，由对应的 vtable 分支负责解引用与析构。
fn build_memo_payload<F: 'static>(
    inline_vtable: &'static MemoVTable,
    boxed_vtable: &'static MemoVTable,
    f: F,
) -> InlineStorage {
    let mut data = InlineStorage::zeroed();
    // SAFETY: 偏移 0 处写入一个指针必然放得下；载荷写在 MEMO_PAYLOAD_OFFSET 处，
    // 由 `InlineStorage::fits` 判定是否内联，放不下则退化为一个 `Box<F>` 指针。
    unsafe {
        if InlineStorage::fits::<F>(MEMO_PAYLOAD_OFFSET) {
            data.write(0, inline_vtable as *const MemoVTable);
            data.write(MEMO_PAYLOAD_OFFSET, f);
        } else {
            data.write(0, boxed_vtable as *const MemoVTable);
            data.write(MEMO_PAYLOAD_OFFSET, Box::new(f));
        }
    }
    data
}

// --- Effect ---

/// 创建一个 effect：立即运行一次 `f`，之后每当它读过的任一 signal 变化就重跑。
///
/// - 依赖是**动态**的：每次运行都会重新收集，上一轮读过、这一轮没读的 signal
///   会被自动退订。
/// - 重跑之前会执行本次运行内 [`crate::on_cleanup`] 注册的清理函数，并销毁
///   本次运行创建的子节点。
/// - 在 effect 体内写 signal 是允许的：写入只会入队，等本次运行结束后再统一
///   调度，首次运行与后续重跑的时序完全一致（AUDIT P1 / P15）。
/// - 若干 effect 互相触发对方的依赖会让队列永远不空，运行时会在若干次迭代后
///   panic 并报出最后调度的节点，而不是把线程挂死（AUDIT P13）。
#[track_caller]
pub fn effect<F: Fn() + 'static>(f: F) -> NodeId {
    let thunk = ThunkValue::new_simple(f);
    internal_create_effect(thunk)
}

#[track_caller]
fn internal_create_effect(thunk: ThunkValue) -> NodeId {
    RUNTIME.get_or(Runtime::new).create_effect(thunk)
}

// --- Memo ---

/// 创建一个惰性求值、带相等性门控的派生节点。
///
/// - **惰性**：依赖变化只把它标脏，真正的重算发生在下一次读取（或下游 effect
///   被调度）时。
/// - **门控**：重算后用 `PartialEq` 与旧值比较，只有真的变了才通知下游。
///   一条 memo 链因此能把上游的抖动挡在中途（见 [`signal`] 的门控表）。
/// - 计算闭包拿到的 `Option<&T>` 是**上一次的结果**，首次计算时为 `None`。
///   它是借来的，不是克隆来的 —— 需要拥有一份请自己 `clone`（AUDIT P9）。
///
/// # 契约
///
/// 不允许在 `f` 内部读取这个 memo 自己：旧值在 `f` 执行期间被移出了节点，
/// 此时节点里放的是占位值，读回来会是 `None`。旧值请从参数拿。
#[track_caller]
pub fn memo<T, F>(f: F) -> NodeId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    let id = RUNTIME.get_or(Runtime::new).register_node();
    internal_init_memo::<T, F>(id, f);
    id
}

#[inline(never)]
fn internal_init_memo<T, F>(id: NodeId, f: F)
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    let data = build_memo_payload(
        &MemoInlineVTable::<T, F>::VTABLE,
        &MemoBoxedVTable::<T, F>::VTABLE,
        f,
    );

    // SAFETY: 载荷由 `build_memo_payload` 按 `MemoVTable` 约定的布局构造。
    unsafe { RUNTIME.get_or(Runtime::new).initialize_memo(id, data) };
}

/// 创建一个惰性求值但**不做相等性门控**的派生节点。
///
/// 与 [`memo`] 的唯一区别就在门控：`T` 只有 `'static` 约束，运行时没有
/// `PartialEq` 可用，因此每一次重算都会通知下游，哪怕算出来的值和上次一样
/// （AUDIT P10）。它换来的是对 `T` 不作任何要求 —— 这正是 `silex_core` 里
/// `Signal::derive` 需要的：任意闭包都能包成一个可读节点。
///
/// 值本身仍然是缓存的：没有依赖变化时读它不会重新执行闭包。
/// 需要“值没变就别惊动下游”，请改用 [`memo`]。
#[track_caller]
pub fn register_derived<T: 'static>(f: Box<dyn Fn() -> T>) -> NodeId {
    let id = RUNTIME.get_or(Runtime::new).register_node();
    internal_init_derived::<T>(id, f);
    id
}

#[inline(never)]
fn internal_init_derived<T: 'static>(id: NodeId, f: Box<dyn Fn() -> T>) {
    // `Box<dyn Fn() -> T>` 是两个机器字的胖指针，恰好放得下；
    // `DerivedVTable` 只有内联这一种布局，所以这里必须真的内联。
    const {
        assert!(
            InlineStorage::fits::<Box<dyn Fn()>>(MEMO_PAYLOAD_OFFSET),
            "derived payload must fit inline"
        );
    }
    let mut data = InlineStorage::zeroed();
    // SAFETY: 上面的 const 断言保证了胖指针放得下；布局与 `DerivedVTable` 一致。
    unsafe {
        data.write(0, &DerivedVTable::<T>::VTABLE as *const MemoVTable);
        data.write(MEMO_PAYLOAD_OFFSET, f);
    }

    // SAFETY: 载荷按 `DerivedVTable` 约定的布局构造。
    unsafe { RUNTIME.get_or(Runtime::new).initialize_memo(id, data) };
}

struct MemoInlineVTable<T, F>(PhantomData<(T, F)>);
impl<T: Clone + PartialEq + 'static, F: Fn(Option<&T>) -> T + 'static> MemoInlineVTable<T, F> {
    const VTABLE: MemoVTable = MemoVTable {
        // 旧值按引用透传给用户闭包：这里克隆一次、运行时再克隆两次，
        // 是每次重算三次深拷贝的来源，而闭包用不用 `old` 都要付（AUDIT P9）。
        compute: FuncPtr::new(|ptr, old| {
            let f = unsafe { &*(ptr as *const F) };
            let new_t = f(old.and_then(|any| any.downcast_ref::<T>()));
            AnyValue::new_reactive(new_t)
        }),
        drop: FuncPtr::new(|ptr| unsafe { drop_in_place(ptr as *mut F) }),
    };
}

struct MemoBoxedVTable<T, F>(PhantomData<(T, F)>);
impl<T: Clone + PartialEq + 'static, F: Fn(Option<&T>) -> T + 'static> MemoBoxedVTable<T, F> {
    const VTABLE: MemoVTable = MemoVTable {
        compute: FuncPtr::new(|ptr, old| {
            let f = unsafe { &**(ptr as *const Box<F>) };
            let new_t = f(old.and_then(|any| any.downcast_ref::<T>()));
            AnyValue::new_reactive(new_t)
        }),
        drop: FuncPtr::new(|ptr| unsafe { drop_in_place(ptr as *mut Box<F>) }),
    };
}

struct DerivedVTable<T>(PhantomData<T>);
impl<T: 'static> DerivedVTable<T> {
    const VTABLE: MemoVTable = MemoVTable {
        compute: FuncPtr::new(|ptr, _| {
            let f = unsafe { &**(ptr as *const Box<dyn Fn() -> T>) };
            AnyValue::new(f())
        }),
        drop: FuncPtr::new(|ptr| unsafe { drop_in_place(ptr as *mut Box<dyn Fn() -> T>) }),
    };
}

/// 读取一个 [`register_derived`] 节点的当前值（必要时先重算），并追踪依赖。
///
/// 与 [`try_get_signal`] **完全等价** —— 函数体就是一行转发。名字暗示的
/// “运行一个派生计算” 也是误导：读取只在节点确实变脏时才驱动重算，
/// 干净的节点直接返回缓存值。请改用 [`try_get_signal`]。
#[deprecated(
    since = "0.1.0-beta.10",
    note = "改用 `try_get_signal`：两者行为完全相同，`run_derived` 只是一个别名"
)]
pub fn run_derived<T: Clone + 'static>(id: NodeId) -> Option<T> {
    try_get_signal(id)
}

// --- Signal ---

/// 创建一个 signal（响应式图的根），返回它的节点句柄。
///
/// # 相等性门控策略
///
/// 整个 crate 只有一条规则，这里写死它（AUDIT P10）：
///
/// | 节点 | 何时通知下游 | 原因 |
/// |---|---|---|
/// | [`signal`] | **每一次成功的写入** | 值只有 `T: 'static`，没有 `PartialEq` 可用 |
/// | [`memo`] | 仅当新值 `!=` 旧值 | 签名要求 `T: Clone + PartialEq`，能比较 |
/// | [`register_derived`] | **每一次重算** | 值只有 `T: 'static`，无法比较 |
///
/// 也就是说 signal 是“无门控”的：写入相同的值同样会把全部下游重跑一遍。
/// 需要门控请用 [`set_signal_if_changed`]（要求 `T: PartialEq`），或者把
/// 下游包一层 [`memo`] —— memo 会把重复的值挡在自己这一层。
#[track_caller]
pub fn signal<T: 'static>(value: T) -> NodeId {
    internal_create_signal(AnyValue::new(value))
}

#[track_caller]
fn internal_create_signal(val: AnyValue) -> NodeId {
    RUNTIME.get_or(Runtime::new).create_signal(val)
}

/// 读取一个 signal / memo / derived 的当前值并**追踪依赖**。
///
/// 在 effect 或 memo 的计算闭包里调用会把该节点登记为当前节点的依赖。
/// 节点不存在、或里面存放的不是 `T` 时返回 `None`。
///
/// 读取一个 memo 可能会驱动它的重算（惰性求值），因此这次调用可能同步执行
/// 用户代码。
pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME
        .get()?
        .get_signal_value(id)?
        .downcast_ref::<T>()
        .cloned()
}

/// 同 [`try_get_signal`]，但**不**登记依赖。
pub fn try_get_signal_untracked<T: Clone + 'static>(id: NodeId) -> Option<T> {
    let rt = RUNTIME.get()?;
    rt.get_signal_value_untracked(id)?
        .downcast_ref::<T>()
        .cloned()
}

/// 一次 signal 写入的结果。
///
/// 存在的理由：之前 `update_signal` 无论成功与否都返回 `()`，类型不匹配时
/// 闭包不执行、值不变，却照样递增版本号并把全部下游重跑一遍（AUDIT P12）。
/// 现在失败被明确表达出来，且失败不再产生任何失效。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// 值已被闭包改写，下游已被失效。
    Updated,
    /// 新值与当前值相等，因此什么都没做 —— 只有显式做相等性门控的写入
    /// （[`set_signal_if_changed`]）才会返回它。
    Unchanged,
    /// 节点不存在、已销毁，或根本不是一个 signal。
    NoSuchSignal,
    /// 节点确实是一个 signal，但里面存放的不是 `T`。
    TypeMismatch,
    /// 该 signal 的值正被外层 update 闭包借出 —— 在 update 闭包内写同一个
    /// signal 是不被支持的（见 [`crate::update_signal`] 的契约）。
    Reentrant,
}

impl UpdateOutcome {
    /// 是否真的写进去了（只有 [`UpdateOutcome::Updated`] 为 `true`）。
    #[inline(always)]
    pub fn is_updated(self) -> bool {
        matches!(self, Self::Updated)
    }
}

/// 就地修改一个 signal 的值并失效下游。
///
/// 写入失败（节点已销毁、类型不匹配、重入）时什么都不会发生 —— 既不改值，
/// 也不递增版本号，也不触发下游。其中类型不匹配是纯粹的编程错误，debug
/// 构建下会断言失败；需要自己处理失败请改用 [`try_update_signal`]。
///
/// # 相等性
///
/// **成功的写入一律触发下游，不做任何相等性比较**（AUDIT P10）—— 哪怕 `f`
/// 什么都没改，或者改成了和原来相同的值。这是有意的：`f` 拿到的是 `&mut T`，
/// 运行时无从得知它改了什么，而为了比较去克隆一份旧值，代价要由所有写入
/// 承担。需要“值不变就不通知”请改用 [`set_signal_if_changed`]。
///
/// 相关的门控策略见 [`signal`]。
///
/// # 契约
///
/// 不允许在 `f` 内部访问同一个 signal（读或写都不行）：值在 `f` 执行期间被
/// 移出了节点，此时节点里放的是占位值。
#[inline(always)]
pub fn update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) {
    let outcome = try_update_signal::<T>(id, f);
    debug_assert!(
        outcome != UpdateOutcome::TypeMismatch,
        "update_signal: 节点 {id:?} 里存放的不是 {}，本次更新被丢弃",
        std::any::type_name::<T>()
    );
}

/// 与 [`update_signal`] 相同，但把结果交还给调用方而不是断言。
#[inline(never)]
pub fn try_update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) -> UpdateOutcome {
    let mut f = Some(f);
    // 只读、或只写既有节点的路径一律用 `get()`：没有运行时就没有节点，
    // 不该仅仅为了报告“查无此节点”而把整个运行时建起来（AUDIT P19.9）。
    let Some(rt) = RUNTIME.get() else {
        return UpdateOutcome::NoSuchSignal;
    };
    let applied = rt.update_signal_untyped(id, &mut |any_val| {
        let Some(val) = any_val.downcast_mut::<T>() else {
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
    });

    match applied {
        Ok(true) => UpdateOutcome::Updated,
        Ok(false) => UpdateOutcome::TypeMismatch,
        Err(SignalBorrowError::Missing) => UpdateOutcome::NoSuchSignal,
        Err(SignalBorrowError::Reentrant) => UpdateOutcome::Reentrant,
    }
}

/// 只在新值与当前值**不相等**时才写入并失效下游。
///
/// signal 本身不做相等性门控（见 [`signal`]）。需要门控的调用方在这里显式
/// 付出一次 `PartialEq::eq` 的代价，而不是让所有写入都为此付费。
///
/// 值相等时返回 [`UpdateOutcome::Unchanged`]：值不变、版本号不变、下游不动。
pub fn set_signal_if_changed<T: PartialEq + 'static>(id: NodeId, value: T) -> UpdateOutcome {
    let mut incoming = Some(value);
    let mut equal = false;
    let Some(rt) = RUNTIME.get() else {
        return UpdateOutcome::NoSuchSignal;
    };
    let applied = rt.update_signal_untyped(id, &mut |any_val| {
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
    });

    match applied {
        Ok(true) => UpdateOutcome::Updated,
        Ok(false) if equal => UpdateOutcome::Unchanged,
        Ok(false) => UpdateOutcome::TypeMismatch,
        Err(SignalBorrowError::Missing) => UpdateOutcome::NoSuchSignal,
        Err(SignalBorrowError::Reentrant) => UpdateOutcome::Reentrant,
    }
}

/// 该句柄是否仍指向一个活着的 signal（含 memo / derived）。
pub fn is_signal_valid(id: NodeId) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.storage
        .reactive
        .get(id)
        .is_some_and(|n| n.signal.is_some())
}

/// 把 `id` 登记为当前运行中节点的依赖，但不读取它的值。
///
/// 用于“只关心变化、不关心值”的场景。当前没有正在运行的节点时什么都不做。
pub fn track_signal(id: NodeId) {
    if let Some(rt) = RUNTIME.get() {
        rt.track_dependency(id);
    }
}

/// [`track_signal`] 的批量版本，只走一遍当前节点的查找。
pub fn track_signals_batch(ids: &[NodeId]) {
    if let Some(rt) = RUNTIME.get() {
        rt.track_dependencies(ids);
    }
}

/// 手工失效一个 signal 的下游，不改动它的值。
///
/// 配合 [`try_update_signal_silent`]：先静默写入若干次，最后统一通知一次。
pub fn notify_signal(id: NodeId) {
    if let Some(rt) = RUNTIME.get() {
        rt.notify_update(id);
    }
}

/// 借用 signal 的当前值（追踪依赖），省掉 [`try_get_signal`] 的那次克隆。
///
/// `f` 执行期间不要重入运行时去销毁这个节点。
pub fn try_with_signal<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME
        .get()?
        .get_signal_value(id)?
        .downcast_ref::<T>()
        .map(f)
}

/// 同 [`try_with_signal`]，但**不**登记依赖。
pub fn try_with_signal_untracked<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    let rt = RUNTIME.get()?;
    rt.get_signal_value_untracked(id)?
        .downcast_ref::<T>()
        .map(f)
}

/// 就地修改一个 signal 的值，但**不**调度下游 —— 调用方需要自己在合适的时机
/// 调用 [`notify_signal`]。版本号照常递增，否则下游的 `Check` 会误判“依赖没变”。
///
/// 类型不匹配时值不变，此时版本号也不会被递增（AUDIT P12）。
pub fn try_update_signal_silent<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    let rt = RUNTIME.get()?;
    let mut out = None;
    // 第二个分量即“请递增版本号”：只有类型相符、闭包真的跑过才递增。
    let _ = rt.with_signal_value_mut(id, |value| match value.downcast_mut::<T>() {
        Some(typed) => {
            out = Some(f(typed));
            ((), true)
        }
        None => ((), false),
    });
    out
}

// --- Storage ---

/// 把一个值交给运行时保管，返回它的句柄。
///
/// 与 signal 的区别：它**不是**响应式的 —— 没有订阅者，读写都不会触发任何
/// 调度。它的生命周期与所属的 scope 绑定（父节点销毁时一并销毁）。
#[track_caller]
pub fn store_value<T: 'static>(value: T) -> NodeId {
    internal_store_value(AnyValue::new(value))
}

#[track_caller]
fn internal_store_value(val: AnyValue) -> NodeId {
    RUNTIME.get_or(Runtime::new).store_value(val)
}

/// 借用一个 [`store_value`] 的值。
pub fn try_with_stored_value<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    let rt = RUNTIME.get()?;
    rt.get_stored_value(id)?.downcast_ref::<T>().map(f)
}

/// 就地修改一个 [`store_value`] 的值。不触发任何调度。
pub fn try_update_stored_value<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    let rt = RUNTIME.get()?;
    let val = rt.get_stored_value_mut(id)?;
    let val = val.downcast_mut::<T>()?;
    Some(f(val))
}

/// 该句柄是否仍指向一个活着的 [`store_value`]。
pub fn is_stored_value_valid(id: NodeId) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::StoredValue(_)))
}

/// 把一个类型擦除的闭包交给运行时保管（供上层框架做去泛型化用）。
#[track_caller]
pub fn register_closure(f: Box<dyn Any>) -> NodeId {
    RUNTIME.get_or(Runtime::new).create_closure(f)
}

/// 按具体类型借用一个 [`register_closure`] 保管的闭包。
pub fn try_with_closure<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    let rt = RUNTIME.get()?;
    let extra = rt.storage.extras.get(id)?;
    if let ExtraData::Closure(c) = extra {
        c.f.downcast_ref::<T>().map(f)
    } else {
        None
    }
}

/// 该句柄是否仍指向一个活着的 [`register_closure`]。
pub fn is_closure_valid(id: NodeId) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::Closure(_)))
}

/// 保管一段定长的类型擦除载荷（[`RawOpBuffer`]），供上层框架传递操作描述。
#[track_caller]
pub fn register_op(buffer: RawOpBuffer) -> NodeId {
    RUNTIME.get_or(Runtime::new).create_op(buffer)
}

/// 借用一个 [`register_op`] 保管的载荷。
pub fn try_with_op<R>(id: NodeId, f: impl FnOnce(&RawOpBuffer) -> R) -> Option<R> {
    let rt = RUNTIME.get()?;
    let extra = rt.storage.extras.get(id)?;
    if let ExtraData::Op(op) = extra {
        Some(f(&op.0))
    } else {
        None
    }
}

/// 该句柄是否仍指向一个活着的 [`register_op`]。
pub fn is_op_valid(id: NodeId) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::Op(_)))
}

// --- Callback API ---

/// 注册一个类型擦除的回调，返回可以到处传递的句柄。
///
/// 回调的生命周期与所属 scope 绑定：scope 销毁后 [`invoke_callback`] 变成 no-op。
#[track_caller]
pub fn register_callback<F>(f: F) -> NodeId
where
    F: Fn(Box<dyn Any>) + 'static,
{
    internal_register_callback(Rc::new(f))
}

#[track_caller]
fn internal_register_callback(f: Rc<dyn Fn(Box<dyn Any>)>) -> NodeId {
    RUNTIME.get_or(Runtime::new).register_callback_untyped(f)
}

/// 调用一个 [`register_callback`] 注册的回调。句柄已失效时什么都不做。
pub fn invoke_callback(id: NodeId, arg: Box<dyn Any>) {
    let Some(rt) = RUNTIME.get() else {
        return;
    };
    if let Some(extra) = rt.storage.extras.get(id)
        && let ExtraData::Callback(data) = extra
    {
        (data.f)(arg);
    }
}

/// 该句柄是否仍指向一个活着的 [`register_callback`]。
pub fn is_callback_valid(id: NodeId) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::Callback(_)))
}

// --- NodeRef API ---

/// 注册一个“稍后填充”的宿主元素引用（DOM 节点等），初始为空。
#[track_caller]
pub fn register_node_ref() -> NodeId {
    internal_register_node_ref()
}

#[track_caller]
fn internal_register_node_ref() -> NodeId {
    RUNTIME.get_or(Runtime::new).register_node_ref()
}

/// 取出 [`register_node_ref`] 里存放的元素（尚未 [`set_node_ref`] 时为 `None`）。
pub fn get_node_ref<T: Clone + 'static>(id: NodeId) -> Option<T> {
    let rt = RUNTIME.get()?;
    let extra = rt.storage.extras.get(id)?;
    if let ExtraData::NodeRef(data) = extra {
        let element = data.element.as_ref()?;
        element.downcast_ref::<T>().cloned()
    } else {
        None
    }
}

/// 填充一个 [`register_node_ref`]。句柄已失效时什么都不做。
pub fn set_node_ref<T: 'static>(id: NodeId, element: T) {
    let Some(rt) = RUNTIME.get() else {
        return;
    };
    if let Some(extra) = rt.storage.extras.get_mut(id)
        && let ExtraData::NodeRef(data) = extra
    {
        data.element = Some(Box::new(element));
    }
}

/// 该句柄是否仍指向一个活着的 [`register_node_ref`]。
pub fn is_node_ref_valid(id: NodeId) -> bool {
    let Some(rt) = RUNTIME.get() else {
        return false;
    };
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::NodeRef(_)))
}

/// 直接借出 stored value 内部的引用，跳过 `Clone`。
///
/// # Safety
///
/// 返回的 `&'static T` 是**伪造**的：数据实际存活在运行时的 arena 里，与
/// `'static` 毫无关系。调用方必须保证在该引用的整个使用期间，下列操作一次都
/// 不发生 —— 任何一条都会让引用悬垂，读它即为未定义行为（AUDIT P6）：
///
/// - [`dispose`](crate::dispose) 该节点或它的任一祖先（释放 arena 槽位）；
/// - [`try_update_stored_value`] 写入一个新值（旧值被 drop）；
/// - 任何会重入运行时并执行用户代码的调用（effect 体、cleanup、`batch` 收尾），
///   因为用户代码可以做上面两件事中的任意一件。
///
/// 换句话说：把返回值当作一个只在“紧接着的、不重入运行时的表达式”里有效的
/// 借用来用。需要更长的存活期就改用 [`try_with_stored_value`]（闭包内访问）
/// 或克隆一份出来。
pub unsafe fn try_get_stored_value_ref<T: 'static>(id: NodeId) -> Option<&'static T> {
    let rt = RUNTIME.get()?;
    let any_val = rt.get_stored_value(id)?;
    any_val.downcast_ref::<T>()
}

/// 直接借出 signal 当前值的引用（不追踪依赖），跳过 `Clone`。
///
/// # Safety
///
/// 与 [`try_get_stored_value_ref`] 完全相同的契约，且 signal 多两条失效来源：
///
/// - `update_signal` / `try_update_signal_silent` 会把值**移出**节点交给用户
///   闭包，期间节点里放的是占位值；
/// - memo 重算后的 `commit_update` 会整体替换掉 signal 的值。
///
/// 本函数自身还会驱动一次惰性求值（memo 的 `update_if_necessary`），这条路径
/// 上可能同步执行下游 effect —— 也就是说**在返回之前**就已经跑过用户代码了。
/// 拿到引用之后请立刻用掉。
pub unsafe fn try_get_signal_value_ref<T: 'static>(id: NodeId) -> Option<&'static T> {
    let rt = RUNTIME.get()?;
    let any_val = rt.get_signal_value_untracked(id)?;
    any_val.downcast_ref::<T>()
}
