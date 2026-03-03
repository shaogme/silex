use crate::core::arena::Index as NodeId;
use crate::core::value::{AnyValue, ThunkValue};
use crate::runtime::RUNTIME;
use std::any::{Any, TypeId};

// --- Context ---

pub fn provide_context<T: 'static>(value: T) {
    internal_provide_context(TypeId::of::<T>(), Box::new(value));
}

fn internal_provide_context(key: TypeId, value: Box<dyn Any>) {
    RUNTIME.with(|rt| rt.provide_context(key, value));
}

pub fn use_context<T: Clone + 'static>() -> Option<T> {
    internal_use_context(TypeId::of::<T>())?
        .downcast_ref::<T>()
        .cloned()
}

fn internal_use_context(key: TypeId) -> Option<&'static dyn Any> {
    RUNTIME.with(|rt| {
        // Safe: Runtime context value is 'static
        let val = rt.use_context_raw(key)?;
        Some(unsafe { &*(val as *const dyn Any) })
    })
}

// --- Effect ---

#[track_caller]
pub fn effect<F: Fn() + 'static>(f: F) -> NodeId {
    let thunk = ThunkValue::new_simple(f);
    internal_create_effect(thunk)
}

fn internal_create_effect(thunk: ThunkValue) -> NodeId {
    RUNTIME.with(|rt| rt.create_effect(thunk))
}

// --- Memo ---

#[track_caller]
pub fn memo<T, F>(f: F) -> NodeId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    let initial_value = RUNTIME.with(|rt| rt.untrack(|| f(None)));

    let thunk = ThunkValue::new(move |rt_ptr: *const ()| {
        let rt = unsafe { &*(rt_ptr as *const crate::runtime::Runtime) };
        rt.update_memo_core(rt.current_owner().unwrap(), |old_any| {
            let old_t = old_any.and_then(|any| any.downcast_ref::<T>().cloned());
            let new_t = f(old_t.as_ref());
            AnyValue::new_reactive(new_t)
        });
    });

    internal_create_memo(AnyValue::new_reactive(initial_value), thunk)
}

#[track_caller]
pub fn register_derived<T: 'static>(f: Box<dyn Fn() -> T>) -> NodeId {
    let initial_value = RUNTIME.with(|rt| rt.untrack(|| f()));

    let thunk = ThunkValue::new(move |rt_ptr: *const ()| {
        let rt = unsafe { &*(rt_ptr as *const crate::runtime::Runtime) };
        rt.update_memo_core(rt.current_owner().unwrap(), |_| AnyValue::new(f()));
    });

    internal_create_memo(AnyValue::new(initial_value), thunk)
}

fn internal_create_memo(initial: AnyValue, thunk: ThunkValue) -> NodeId {
    RUNTIME.with(|rt| rt.create_memo_node_untyped(initial, thunk))
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
    RUNTIME.with(|rt| rt.create_signal(val))
}

pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        rt.prepare_read(id);
        rt.storage
            .signals
            .get(id)?
            .value
            .downcast_ref::<T>()
            .cloned()
    })
}

pub fn try_get_signal_untracked<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        rt.prepare_read_untracked(id);
        rt.storage
            .signals
            .get(id)?
            .value
            .downcast_ref::<T>()
            .cloned()
    })
}

pub fn update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) {
    let mut f = Some(f);
    RUNTIME.with(|rt| {
        rt.update_signal_untyped(id, &mut |any_val| {
            if let Some(f) = f.take() {
                if let Some(val) = any_val.downcast_mut::<T>() {
                    f(val);
                }
            }
        });
    })
}

pub fn is_signal_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.storage.signals.contains_key(id))
}

pub fn track_signal(id: NodeId) {
    RUNTIME.with(|rt| rt.track_dependency(id))
}

pub fn track_signals_batch(ids: &[NodeId]) {
    RUNTIME.with(|rt| rt.track_dependencies(ids))
}

pub fn notify_signal(id: NodeId) {
    RUNTIME.with(|rt| rt.notify_update(id))
}

pub fn try_with_signal<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        rt.prepare_read(id);
        let signal = rt.storage.signals.get(id)?;
        signal.value.downcast_ref::<T>().map(f)
    })
}

pub fn try_with_signal_untracked<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        rt.prepare_read_untracked(id);
        let signal = rt.storage.signals.get(id)?;
        signal.value.downcast_ref::<T>().map(f)
    })
}

pub fn try_update_signal_silent<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    RUNTIME.with(|rt| {
        let signal = rt.storage.signals.get_mut(id)?;
        let val = signal.value.downcast_mut::<T>()?;
        signal.version = signal.version.wrapping_add(1);
        Some(f(val))
    })
}

// --- Storage ---

#[track_caller]
pub fn store_value<T: 'static>(value: T) -> NodeId {
    internal_store_value(AnyValue::new(value))
}

fn internal_store_value(val: AnyValue) -> NodeId {
    RUNTIME.with(|rt| rt.store_value(val))
}

pub fn try_with_stored_value<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        let data = rt.storage.stored_values.get(id)?;
        data.value.downcast_ref::<T>().map(f)
    })
}

pub fn try_update_stored_value<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    RUNTIME.with(|rt| {
        let data = rt.storage.stored_values.get_mut(id)?;
        let val = data.value.downcast_mut::<T>()?;
        Some(f(val))
    })
}

pub fn is_stored_value_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.storage.stored_values.contains_key(id))
}

pub fn register_closure(f: Box<dyn Any>) -> NodeId {
    RUNTIME.with(|rt| rt.create_closure(f))
}

pub fn try_with_closure<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        let data = rt.storage.closures.get(id)?;
        data.f.downcast_ref::<T>().map(f)
    })
}

pub fn is_closure_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.storage.closures.contains_key(id))
}

pub fn register_op(buffer: crate::RawOpBuffer) -> NodeId {
    RUNTIME.with(|rt| rt.create_op(buffer))
}

pub fn try_with_op<R>(id: NodeId, f: impl FnOnce(&crate::RawOpBuffer) -> R) -> Option<R> {
    RUNTIME.with(|rt| rt.storage.ops.get(id).map(|data| f(&data.0)))
}

pub fn is_op_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.storage.ops.contains_key(id))
}

// --- Callback API ---

#[track_caller]
pub fn register_callback<F>(f: F) -> NodeId
where
    F: Fn(Box<dyn Any>) + 'static,
{
    internal_register_callback(std::rc::Rc::new(f))
}

fn internal_register_callback(f: std::rc::Rc<dyn Fn(Box<dyn Any>)>) -> NodeId {
    RUNTIME.with(|rt| rt.register_callback_untyped(f))
}

pub fn invoke_callback(id: NodeId, arg: Box<dyn Any>) {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.storage.callbacks.get(id) {
            (data.f)(arg);
        }
    })
}

pub fn is_callback_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.storage.callbacks.contains_key(id))
}

// --- NodeRef API ---

#[track_caller]
pub fn register_node_ref() -> NodeId {
    internal_register_node_ref()
}

fn internal_register_node_ref() -> NodeId {
    RUNTIME.with(|rt| rt.register_node_ref())
}

pub fn get_node_ref<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        let data = rt.storage.node_refs.get(id)?;
        let element = data.element.as_ref()?;
        element.downcast_ref::<T>().cloned()
    })
}

pub fn set_node_ref<T: 'static>(id: NodeId, element: T) {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.storage.node_refs.get_mut(id) {
            data.element = Some(Box::new(element));
        }
    })
}

pub fn is_node_ref_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.storage.node_refs.contains_key(id))
}
