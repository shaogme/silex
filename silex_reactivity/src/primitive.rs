use crate::core::arena::Index as NodeId;
use crate::runtime::RUNTIME;
use std::any::Any;

// --- Context ---

pub fn provide_context<T: 'static>(value: T) {
    RUNTIME.with(|rt| rt.provide_context(value));
}

pub fn use_context<T: Clone + 'static>() -> Option<T> {
    RUNTIME.with(|rt| rt.use_context::<T>())
}

// --- Effect ---

#[track_caller]
pub fn effect<F: Fn() + 'static>(f: F) -> NodeId {
    RUNTIME.with(|rt| rt.create_effect(f))
}

// --- Memo ---

#[track_caller]
pub fn memo<T, F>(f: F) -> NodeId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    RUNTIME.with(|rt| rt.create_memo(f))
}

#[track_caller]
pub fn register_derived<T: 'static>(f: Box<dyn Fn() -> T>) -> NodeId {
    RUNTIME.with(|rt| rt.register_derived(f))
}

pub fn run_derived<T: Clone + 'static>(id: NodeId) -> Option<T> {
    try_get_signal(id)
}

// --- Signal ---

#[track_caller]
pub fn signal<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| rt.create_signal(value))
}

pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        rt.track_dependency(id);
        rt.update_if_necessary(id);

        if let Some(signal) = rt.storage.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(val.clone());
            } else {
                eprintln!("Type mismatch in try_get_signal");
            }
        }
        None
    })
}

pub fn try_get_signal_untracked<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        rt.update_if_necessary(id);

        if let Some(signal) = rt.storage.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(val.clone());
            } else {
                eprintln!("Type mismatch in try_get_signal_untracked");
            }
        }
        None
    })
}

pub fn update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) {
    RUNTIME.with(|rt| {
        {
            if let Some(signal) = rt.storage.signals.get_mut(id) {
                signal.version = signal.version.wrapping_add(1);
                if let Some(val) = signal.value.downcast_mut::<T>() {
                    f(val);
                } else {
                    eprintln!("Type mismatch in update_signal");
                    return;
                }
            } else {
                return;
            }
        }
        rt.queue_dependents(id);
        if rt.scheduler.batch_depth.get() == 0 {
            rt.run_queue();
        }
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
    RUNTIME.with(|rt| {
        rt.queue_dependents(id);
        if rt.scheduler.batch_depth.get() == 0 {
            rt.run_queue();
        }
    })
}

pub fn try_with_signal<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        rt.track_dependency(id);
        rt.update_if_necessary(id);

        if let Some(signal) = rt.storage.signals.get(id)
            && let Some(val) = signal.value.downcast_ref::<T>()
        {
            return Some(f(val));
        }
        None
    })
}

pub fn try_with_signal_untracked<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        rt.update_if_necessary(id);
        if let Some(signal) = rt.storage.signals.get(id)
            && let Some(val) = signal.value.downcast_ref::<T>()
        {
            return Some(f(val));
        }
        None
    })
}

pub fn try_update_signal_silent<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    RUNTIME.with(|rt| {
        if let Some(signal) = rt.storage.signals.get_mut(id)
            && let Some(val) = signal.value.downcast_mut::<T>()
        {
            signal.version = signal.version.wrapping_add(1);
            return Some(f(val));
        }
        None
    })
}

// --- Storage ---

#[track_caller]
pub fn store_value<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| rt.store_value(value))
}

pub fn try_with_stored_value<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.storage.stored_values.get(id)
            && let Some(val) = data.value.downcast_ref::<T>()
        {
            return Some(f(val));
        }
        None
    })
}

pub fn try_update_stored_value<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.storage.stored_values.get_mut(id)
            && let Some(val) = data.value.downcast_mut::<T>()
        {
            return Some(f(val));
        }
        None
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
        if let Some(data) = rt.storage.closures.get(id)
            && let Some(val) = data.f.downcast_ref::<T>()
        {
            return Some(f(val));
        }
        None
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
    RUNTIME.with(|rt| rt.register_callback(f))
}

pub fn invoke_callback(id: NodeId, arg: Box<dyn Any>) {
    RUNTIME.with(|rt| {
        let callback = rt.storage.callbacks.get(id).map(|data| data.f.clone());
        if let Some(f) = callback {
            f(arg);
        }
    })
}

pub fn is_callback_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.storage.callbacks.get(id).is_some())
}

// --- NodeRef API ---

#[track_caller]
pub fn register_node_ref() -> NodeId {
    RUNTIME.with(|rt| rt.register_node_ref())
}

pub fn get_node_ref<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.storage.node_refs.get(id)
            && let Some(ref element) = data.element
        {
            return element.downcast_ref::<T>().cloned();
        }
        None
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
    RUNTIME.with(|rt| rt.storage.node_refs.get(id).is_some())
}
