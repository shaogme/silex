use crate::{
    RawOpBuffer,
    core::{
        FuncPtr,
        arena::Index as NodeId,
        value::{AnyValue, ThunkValue},
    },
    runtime::{MemoVTable, RUNTIME, Runtime, storage::ExtraData},
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

#[inline(always)]
pub fn update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) {
    internal_update_signal::<T>(id, f);
}

#[inline(never)]
fn internal_update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) {
    let mut f = Some(f);
    let rt = RUNTIME.get_or(Runtime::new);
    rt.update_signal_untyped(id, &mut |any_val| {
        if let Some(f) = f.take()
            && let Some(val) = any_val.downcast_mut::<T>()
        {
            f(val);
        }
    });
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

pub fn try_update_signal_silent<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    let rt = RUNTIME.get_or(Runtime::new);
    let mut out = None;
    rt.with_signal_value_mut(id, |value| {
        if let Some(typed) = value.downcast_mut::<T>() {
            out = Some(f(typed));
        }
    });
    out
}

// --- Storage ---

#[track_caller]
pub fn store_value<T: 'static>(value: T) -> NodeId {
    internal_store_value(AnyValue::new(value))
}

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

pub fn try_get_stored_value_ref<T: 'static>(id: NodeId) -> Option<&'static T> {
    let rt = RUNTIME.get()?;
    let any_val = rt.get_stored_value(id)?;
    any_val.downcast_ref::<T>()
}

pub fn try_get_signal_value_ref<T: 'static>(id: NodeId) -> Option<&'static T> {
    let rt = RUNTIME.get()?;
    let any_val = rt.get_signal_value_untracked(id)?;
    any_val.downcast_ref::<T>()
}
