use crate::core::FuncPtr;
use crate::core::arena::Index as NodeId;
use crate::core::value::AnyValue;
use crate::runtime::{RUNTIME, UniversalDerivedRunner, UniversalMemoRunner};
use std::any::{Any, TypeId};

// --- Context ---

pub fn provide_context<T: 'static>(value: T) {
    RUNTIME.with(|rt| rt.provide_context(TypeId::of::<T>(), Box::new(value)));
}

pub fn use_context<T: Clone + 'static>() -> Option<T> {
    RUNTIME.with(|rt| {
        rt.use_context_raw(TypeId::of::<T>())?
            .downcast_ref::<T>()
            .cloned()
    })
}

// --- Effect ---

#[track_caller]
pub fn effect<F: Fn() + 'static>(f: F) -> NodeId {
    RUNTIME.with(|rt| rt.create_effect(Box::new(move |_| f())))
}

// --- Memo ---

#[track_caller]
pub fn memo<T, F>(f: F) -> NodeId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    RUNTIME.with(|rt| {
        let initial_value = rt.untrack(|| f(None));

        // --- Static Thunks ---
        unsafe fn compute_thunk<T, F>(data: *mut (), old: Option<AnyValue>) -> AnyValue
        where
            T: Clone + PartialEq + 'static,
            F: Fn(Option<&T>) -> T + 'static,
        {
            let f = unsafe { &*(data as *const F) };
            let old_t = old.and_then(|any| any.downcast_ref::<T>().cloned());
            let new_t = f(old_t.as_ref());
            AnyValue::new_reactive(new_t)
        }

        unsafe fn drop_thunk<F>(data: *mut ()) {
            let _ = unsafe { Box::from_raw(data as *mut F) };
        }

        let data = Box::into_raw(Box::new(f)) as *mut ();

        rt.create_memo_node_raw(
            AnyValue::new_reactive(initial_value),
            Box::new(UniversalMemoRunner {
                data,
                compute: FuncPtr::new(compute_thunk::<T, F>),
                drop: FuncPtr::new(drop_thunk::<F>),
            }),
        )
    })
}

#[track_caller]
pub fn register_derived<T: 'static>(f: Box<dyn Fn() -> T>) -> NodeId {
    RUNTIME.with(|rt| {
        let initial_value = rt.untrack(|| f());

        // --- Static Thunks ---
        unsafe fn compute_thunk<T>(data: *mut ()) -> AnyValue
        where
            T: 'static,
        {
            let f = unsafe { &*(data as *const Box<dyn Fn() -> T>) };
            AnyValue::new(f())
        }

        unsafe fn drop_thunk<T>(data: *mut ()) {
            let _ = unsafe { Box::from_raw(data as *mut Box<dyn Fn() -> T>) };
        }

        let data = Box::into_raw(Box::new(f)) as *mut ();

        rt.create_memo_node_raw(
            AnyValue::new(initial_value),
            Box::new(UniversalDerivedRunner {
                data,
                compute: FuncPtr::new(compute_thunk::<T>),
                drop: FuncPtr::new(drop_thunk::<T>),
            }),
        )
    })
}

pub fn run_derived<T: Clone + 'static>(id: NodeId) -> Option<T> {
    try_get_signal(id)
}

// --- Signal ---

#[track_caller]
pub fn signal<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| rt.create_signal(AnyValue::new(value)))
}

pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        rt.prepare_read(id);
        let signal = rt.storage.signals.get(id)?;
        if let Some(val) = signal.value.downcast_ref::<T>() {
            Some(val.clone())
        } else {
            eprintln!("Type mismatch in try_get_signal");
            None
        }
    })
}

pub fn try_get_signal_untracked<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        rt.prepare_read_untracked(id);
        let signal = rt.storage.signals.get(id)?;
        if let Some(val) = signal.value.downcast_ref::<T>() {
            Some(val.clone())
        } else {
            eprintln!("Type mismatch in try_get_signal_untracked");
            None
        }
    })
}

pub fn update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) {
    let mut f = Some(f);
    RUNTIME.with(|rt| {
        rt.update_signal_untyped(id, &mut |any_val| {
            if let Some(f) = f.take() {
                if let Some(val) = any_val.downcast_mut::<T>() {
                    f(val);
                } else {
                    eprintln!("Type mismatch in update_signal");
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
        if let Some(val) = signal.value.downcast_ref::<T>() {
            Some(f(val))
        } else {
            None
        }
    })
}

pub fn try_with_signal_untracked<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        rt.prepare_read_untracked(id);
        let signal = rt.storage.signals.get(id)?;
        if let Some(val) = signal.value.downcast_ref::<T>() {
            Some(f(val))
        } else {
            None
        }
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
    RUNTIME.with(|rt| rt.store_value(AnyValue::new(value)))
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
