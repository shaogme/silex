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
        compute: FuncPtr::new(|ptr, old| {
            let f = unsafe { &*(ptr as *const F) };
            let old_t = old.and_then(|any| any.downcast_ref::<T>().cloned());
            let new_t = f(old_t.as_ref());
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
            let old_t = old.and_then(|any| any.downcast_ref::<T>().cloned());
            let new_t = f(old_t.as_ref());
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

pub fn run_derived<T: Clone + 'static>(id: NodeId) -> Option<T> {
    try_get_signal(id)
}

// --- Signal ---

#[track_caller]
pub fn signal<T: 'static>(value: T) -> NodeId {
    internal_create_signal(AnyValue::new(value))
}

#[track_caller]
fn internal_create_signal(val: AnyValue) -> NodeId {
    RUNTIME.get_or(Runtime::new).create_signal(val)
}

pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME
        .get()?
        .get_signal_value(id)?
        .downcast_ref::<T>()
        .cloned()
}

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
    /// 节点不存在、已销毁，或根本不是一个 signal。
    NoSuchSignal,
    /// 节点确实是一个 signal，但里面存放的不是 `T`。
    TypeMismatch,
    /// 该 signal 的值正被外层 update 闭包借出 —— 在 update 闭包内写同一个
    /// signal 是不被支持的（见 [`crate::update_signal`] 的契约）。
    Reentrant,
}

impl UpdateOutcome {
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
    let rt = RUNTIME.get_or(Runtime::new);
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

pub fn is_signal_valid(id: NodeId) -> bool {
    let rt = RUNTIME.get_or(Runtime::new);
    rt.storage
        .reactive
        .get(id)
        .is_some_and(|n| n.signal.is_some())
}

pub fn track_signal(id: NodeId) {
    RUNTIME.get_or(Runtime::new).track_dependency(id);
}

pub fn track_signals_batch(ids: &[NodeId]) {
    RUNTIME.get_or(Runtime::new).track_dependencies(ids);
}

pub fn notify_signal(id: NodeId) {
    RUNTIME.get_or(Runtime::new).notify_update(id);
}

pub fn try_with_signal<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME
        .get()?
        .get_signal_value(id)?
        .downcast_ref::<T>()
        .map(f)
}

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
    let rt = RUNTIME.get_or(Runtime::new);
    let mut out = None;
    let applied = rt
        .with_signal_value_mut(id, |value| match value.downcast_mut::<T>() {
            Some(typed) => {
                out = Some(f(typed));
                true
            }
            None => false,
        })
        .unwrap_or(false);

    if applied {
        rt.bump_signal_version(id);
    }
    out
}

// --- Storage ---

#[track_caller]
pub fn store_value<T: 'static>(value: T) -> NodeId {
    internal_store_value(AnyValue::new(value))
}

#[track_caller]
fn internal_store_value(val: AnyValue) -> NodeId {
    RUNTIME.get_or(Runtime::new).store_value(val)
}

pub fn try_with_stored_value<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    let rt = RUNTIME.get()?;
    rt.get_stored_value(id)?.downcast_ref::<T>().map(f)
}

pub fn try_update_stored_value<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    let rt = RUNTIME.get_or(Runtime::new);
    let val = rt.get_stored_value_mut(id)?;
    let val = val.downcast_mut::<T>()?;
    Some(f(val))
}

pub fn is_stored_value_valid(id: NodeId) -> bool {
    let rt = RUNTIME.get_or(Runtime::new);
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::StoredValue(_)))
}

#[track_caller]
pub fn register_closure(f: Box<dyn Any>) -> NodeId {
    RUNTIME.get_or(Runtime::new).create_closure(f)
}

pub fn try_with_closure<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    let rt = RUNTIME.get()?;
    let extra = rt.storage.extras.get(id)?;
    if let ExtraData::Closure(c) = extra {
        c.f.downcast_ref::<T>().map(f)
    } else {
        None
    }
}

pub fn is_closure_valid(id: NodeId) -> bool {
    let rt = RUNTIME.get_or(Runtime::new);
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::Closure(_)))
}

#[track_caller]
pub fn register_op(buffer: RawOpBuffer) -> NodeId {
    RUNTIME.get_or(Runtime::new).create_op(buffer)
}

pub fn try_with_op<R>(id: NodeId, f: impl FnOnce(&RawOpBuffer) -> R) -> Option<R> {
    let rt = RUNTIME.get()?;
    let extra = rt.storage.extras.get(id)?;
    if let ExtraData::Op(op) = extra {
        Some(f(&op.0))
    } else {
        None
    }
}

pub fn is_op_valid(id: NodeId) -> bool {
    let rt = RUNTIME.get_or(Runtime::new);
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::Op(_)))
}

// --- Callback API ---

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

pub fn invoke_callback(id: NodeId, arg: Box<dyn Any>) {
    let rt = RUNTIME.get_or(Runtime::new);
    if let Some(extra) = rt.storage.extras.get(id)
        && let ExtraData::Callback(data) = extra
    {
        (data.f)(arg);
    }
}

pub fn is_callback_valid(id: NodeId) -> bool {
    let rt = RUNTIME.get_or(Runtime::new);
    rt.storage
        .extras
        .get(id)
        .is_some_and(|e| matches!(e, ExtraData::Callback(_)))
}

// --- NodeRef API ---

#[track_caller]
pub fn register_node_ref() -> NodeId {
    internal_register_node_ref()
}

#[track_caller]
fn internal_register_node_ref() -> NodeId {
    RUNTIME.get_or(Runtime::new).register_node_ref()
}

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

pub fn set_node_ref<T: 'static>(id: NodeId, element: T) {
    let rt = RUNTIME.get_or(Runtime::new);
    if let Some(extra) = rt.storage.extras.get_mut(id)
        && let ExtraData::NodeRef(data) = extra
    {
        data.element = Some(Box::new(element));
    }
}

pub fn is_node_ref_valid(id: NodeId) -> bool {
    let rt = RUNTIME.get_or(Runtime::new);
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
