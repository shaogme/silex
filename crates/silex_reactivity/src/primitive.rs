use crate::{
    RawOpBuffer,
    core::{
        FuncPtr,
        arena::Index as NodeId,
        value::{AnyValue, ThunkValue},
    },
    runtime::{MemoVTable, RUNTIME, Runtime, storage::ExtraData},
};
use std::{
    alloc::Layout,
    any::{Any, TypeId},
    marker::PhantomData,
    mem::{align_of, size_of},
    ptr::{drop_in_place, write},
    rc::Rc,
};

// --- Context ---

pub fn provide_context<T: 'static>(value: T) {
    internal_provide_context(TypeId::of::<T>(), Box::new(value));
}

fn internal_provide_context(key: TypeId, value: Box<dyn Any>) {
    RUNTIME.get_or(Runtime::new).provide_context(key, value);
}

pub fn use_context<T: Clone + 'static>() -> Option<T> {
    internal_use_context(TypeId::of::<T>())?
        .downcast_ref::<T>()
        .cloned()
}

fn internal_use_context(key: TypeId) -> Option<&'static dyn Any> {
    let rt = RUNTIME.get()?;
    // Safe: Runtime context value is 'static
    let val = rt.use_context_raw(key)?;
    Some(unsafe { &*(val as *const dyn Any) })
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
    let layout = Layout::new::<F>();
    let fits_inline =
        layout.size() <= 2 * size_of::<usize>() && layout.align() <= align_of::<usize>();

    let mut data = [0usize; 3];
    if fits_inline {
        data[0] = &MemoInlineVTable::<T, F>::VTABLE as *const _ as usize;
        unsafe { write(data.as_mut_ptr().add(1) as *mut F, f) };
    } else {
        data[0] = &MemoBoxedVTable::<T, F>::VTABLE as *const _ as usize;
        let boxed = Box::new(f);
        unsafe { write(data.as_mut_ptr().add(1) as *mut Box<F>, boxed) };
    }

    unsafe { RUNTIME.get_or(Runtime::new).initialize_memo_raw(id, data) };
}

#[track_caller]
pub fn register_derived<T: 'static>(f: Box<dyn Fn() -> T>) -> NodeId {
    let id = RUNTIME.get_or(Runtime::new).register_node();
    internal_init_derived::<T>(id, f);
    id
}

#[inline(never)]
fn internal_init_derived<T: 'static>(id: NodeId, f: Box<dyn Fn() -> T>) {
    let mut data = [0usize; 3];
    data[0] = &DerivedVTable::<T>::VTABLE as *const _ as usize;
    unsafe { write(data.as_mut_ptr().add(1) as *mut Box<dyn Fn() -> T>, f) };

    unsafe { RUNTIME.get_or(Runtime::new).initialize_memo_raw(id, data) };
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
    let val = rt.get_signal_value_mut_silent(id)?;
    let val = val.downcast_mut::<T>()?;
    Some(f(val))
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
