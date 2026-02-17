use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::rc::Rc;

mod arena;
pub use arena::{Arena, Index as NodeId, SparseSecondaryMap};

pub mod algorithm;
use algorithm::NodeState;

mod value;
use value::AnyValue;

mod list;
pub(crate) use list::List;

pub(crate) type NodeList = List<NodeId>;
pub(crate) type DependencyList = List<(NodeId, u32)>;

mod runtime;
use runtime::{CallbackData, NodeRefData, RUNTIME, StoredValueData, run_effect_internal};

// --- Public High-Level API ---

#[track_caller]
pub fn signal<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| rt.register_signal_internal(value))
}

pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        // Track
        rt.track_dependency(id);
        rt.update_if_necessary(id);

        if let Some(signal) = rt.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(val.clone());
            } else {
                eprintln!("Type mismatch in try_get_signal (Signal)");
            }
        } else if let Some(derived) = rt.deriveds.get(id) {
            if let Some(val) = derived.signal.value.downcast_ref::<T>() {
                return Some(val.clone());
            } else {
                eprintln!("Type mismatch in try_get_signal (Derived)");
            }
        }
        None
    })
}

pub fn try_get_signal_untracked<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        rt.update_if_necessary(id);
        if let Some(signal) = rt.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(val.clone());
            } else {
                eprintln!("Type mismatch in try_get_signal_untracked (Signal)");
            }
        } else if let Some(derived) = rt.deriveds.get(id) {
            if let Some(val) = derived.signal.value.downcast_ref::<T>() {
                return Some(val.clone());
            } else {
                eprintln!("Type mismatch in try_get_signal_untracked (Derived)");
            }
        }
        None
    })
}

pub fn update_signal<T: 'static>(id: NodeId, f: impl FnOnce(&mut T)) {
    RUNTIME.with(|rt| {
        {
            if let Some(signal) = rt.signals.get_mut(id) {
                signal.version = signal.version.wrapping_add(1);
                if let Some(val) = signal.value.downcast_mut::<T>() {
                    f(val);
                } else {
                    eprintln!("Type mismatch in update_signal");
                    return;
                }
            } else {
                return; // Dropped
            }
        }
        rt.queue_dependents(id);
        if rt.batch_depth.get() == 0 {
            rt.run_queue();
        }
    })
}

pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    RUNTIME.with(|rt| {
        let depth = rt.batch_depth.get();
        rt.batch_depth.set(depth + 1);

        let result = f();

        rt.batch_depth.set(depth);

        if depth == 0 && !rt.running_queue.get() {
            rt.run_queue();
        }

        result
    })
}

#[track_caller]
pub fn effect<F: Fn() + 'static>(f: F) -> NodeId {
    RUNTIME.with(|rt| {
        let id = rt.register_effect_internal(f);
        run_effect_internal(rt, id);
        id
    })
}

#[track_caller]
pub fn create_scope<F>(f: F) -> NodeId
where
    F: FnOnce(),
{
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        let prev_owner = rt.current_owner.get();
        rt.current_owner.set(Some(id));
        f();
        rt.current_owner.set(prev_owner);
        id
    })
}

pub fn dispose(id: NodeId) {
    RUNTIME.with(|rt| rt.dispose_node_internal(id, true));
}

pub fn on_cleanup(f: impl FnOnce() + 'static) {
    RUNTIME.with(|rt| {
        if let Some(owner) = rt.current_owner.get() {
            rt.aux_mut(owner).cleanups.push(Box::new(f));
        }
    });
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.with(|rt| {
        let prev_owner = rt.current_owner.get();
        rt.current_owner.set(None);
        let t = f();
        rt.current_owner.set(prev_owner);
        t
    })
}

// Provide generic memo creation
#[track_caller]
pub fn memo<T, F>(f: F) -> NodeId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    RUNTIME.with(|rt| {
        let id = rt.register_node();

        // Placeholder data to allow tracking during initial run
        rt.deriveds.insert(
            id,
            runtime::DerivedData {
                signal: runtime::SignalData {
                    value: AnyValue::new(()), // Temporary dummy
                    subscribers: NodeList::Empty,
                    last_tracked_by: None,
                    version: 0,
                },
                effect: runtime::EffectData {
                    computation: None,
                    dependencies: DependencyList::default(),
                    effect_version: 0,
                },
                state: NodeState::Clean,
            },
        );

        let initial_value = {
            let prev_owner = rt.current_owner.get();
            rt.current_owner.set(Some(id));
            let v = f(None);
            rt.current_owner.set(prev_owner);
            v
        };

        // Construct the computation closure
        let computation = move || {
            // Access old value
            let old_value = RUNTIME.with(|rt| {
                if let Some(derived) = rt.deriveds.get(id) {
                    if let Some(val) = derived.signal.value.downcast_ref::<T>() {
                        Some(val.clone())
                    } else {
                        None // Should not happen if types match
                    }
                } else {
                    None
                }
            });

            let new_value = f(old_value.as_ref());

            let mut changed = false;
            if let Some(old) = &old_value {
                if new_value != *old {
                    changed = true;
                }
            } else {
                changed = true;
            }

            if changed {
                RUNTIME.with(|rt| {
                    if let Some(derived) = rt.deriveds.get_mut(id) {
                        derived.signal.value = AnyValue::new(new_value);
                        derived.signal.version = derived.signal.version.wrapping_add(1);
                    }
                    rt.queue_dependents(id);
                });
            }
        };

        // Update the node with real data
        if let Some(data) = rt.deriveds.get_mut(id) {
            data.signal.value = AnyValue::new(initial_value);
            data.effect.computation = Some(Box::new(computation));
        }

        id
    })
}

// Context API exposed
pub fn provide_context_any(key: TypeId, value: Box<dyn Any>) {
    RUNTIME.with(|rt| {
        if let Some(owner) = rt.current_owner.get() {
            let aux = rt.aux_mut(owner);
            if aux.context.is_none() {
                aux.context = Some(HashMap::new());
            }
            if let Some(ctx) = &mut aux.context {
                ctx.insert(key, value);
            }
        }
    })
}

// Better Context API
pub fn provide_context<T: 'static>(value: T) {
    provide_context_any(TypeId::of::<T>(), Box::new(value));
}

pub fn use_context<T: Clone + 'static>() -> Option<T> {
    RUNTIME.with(|rt| {
        // Since graph traversal is needed, we need to be careful with references.
        // Arena supports multiple immutable references.

        let mut current_opt = rt.current_owner.get();

        while let Some(current) = current_opt {
            if let Some(aux) = rt.node_aux.get(current) {
                if let Some(ctx) = &aux.context {
                    if let Some(val) = ctx.get(&TypeId::of::<T>()) {
                        return val.downcast_ref::<T>().cloned();
                    }
                }
            }

            if let Some(node) = rt.graph.get(current) {
                current_opt = node.parent;
            } else {
                current_opt = None;
            }
        }
        None
    })
}

// --- Callback API ---

#[track_caller]
pub fn register_callback<F>(f: F) -> NodeId
where
    F: Fn(Box<dyn Any>) + 'static,
{
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.callbacks.insert(id, CallbackData { f: Rc::new(f) });
        id
    })
}

pub fn invoke_callback(id: NodeId, arg: Box<dyn Any>) {
    RUNTIME.with(|rt| {
        let callback = rt.callbacks.get(id).map(|data| data.f.clone());
        if let Some(f) = callback {
            f(arg);
        }
    })
}

pub fn is_callback_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.callbacks.get(id).is_some())
}

// --- NodeRef API ---

#[track_caller]
pub fn register_node_ref() -> NodeId {
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.node_refs.insert(id, NodeRefData { element: None });
        id
    })
}

pub fn get_node_ref<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.node_refs.get(id) {
            if let Some(ref element) = data.element {
                return element.downcast_ref::<T>().cloned();
            }
        }
        None
    })
}

pub fn set_node_ref<T: 'static>(id: NodeId, element: T) {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.node_refs.get_mut(id) {
            data.element = Some(Box::new(element));
        }
    })
}

pub fn is_node_ref_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.node_refs.get(id).is_some())
}

pub fn track_signal(id: NodeId) {
    RUNTIME.with(|rt| rt.track_dependency(id))
}

pub fn notify_signal(id: NodeId) {
    RUNTIME.with(|rt| {
        rt.queue_dependents(id);
        if rt.batch_depth.get() == 0 {
            rt.run_queue();
        }
    })
}

// --- StoredValue API ---

#[track_caller]
pub fn store_value<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.stored_values.insert(
            id,
            StoredValueData {
                value: AnyValue::new(value),
            },
        );
        id
    })
}

pub fn try_with_stored_value<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.stored_values.get(id) {
            if let Some(val) = data.value.downcast_ref::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

pub fn try_update_stored_value<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.stored_values.get_mut(id) {
            if let Some(val) = data.value.downcast_mut::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

// --- Derived API ---

#[track_caller]
pub fn register_derived<T: 'static>(f: impl Fn() -> T + 'static) -> NodeId {
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.deriveds.insert(
            id,
            runtime::DerivedData {
                signal: runtime::SignalData {
                    value: AnyValue::new(()),
                    subscribers: NodeList::Empty,
                    last_tracked_by: None,
                    version: 0,
                },
                effect: runtime::EffectData {
                    computation: None,
                    dependencies: DependencyList::default(),
                    effect_version: 0,
                },
                state: NodeState::Clean,
            },
        );

        let initial_value = {
            let prev_owner = rt.current_owner.get();
            rt.current_owner.set(Some(id));
            let v = f();
            rt.current_owner.set(prev_owner);
            v
        };

        let computation = move || {
            let new_value = f();
            RUNTIME.with(|rt| {
                if let Some(derived) = rt.deriveds.get_mut(id) {
                    derived.signal.value = AnyValue::new(new_value);
                    derived.signal.version = derived.signal.version.wrapping_add(1);
                }
                rt.queue_dependents(id);
            });
        };

        if let Some(data) = rt.deriveds.get_mut(id) {
            data.signal.value = AnyValue::new(initial_value);
            data.effect.computation = Some(Box::new(computation));
        }
        id
    })
}

pub fn run_derived<T: Clone + 'static>(id: NodeId) -> Option<T> {
    try_get_signal(id)
}

pub fn try_with_signal<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        // Track
        rt.track_dependency(id);
        rt.update_if_necessary(id);

        if let Some(signal) = rt.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(f(val));
            }
        } else if let Some(derived) = rt.deriveds.get(id) {
            if let Some(val) = derived.signal.value.downcast_ref::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

pub fn try_with_signal_untracked<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        rt.update_if_necessary(id);
        if let Some(signal) = rt.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(f(val));
            }
        } else if let Some(derived) = rt.deriveds.get(id) {
            if let Some(val) = derived.signal.value.downcast_ref::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

pub fn try_update_signal_silent<T: 'static, R>(
    id: NodeId,
    f: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    RUNTIME.with(|rt| {
        if let Some(signal) = rt.signals.get_mut(id) {
            if let Some(val) = signal.value.downcast_mut::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

pub fn is_signal_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.signals.get(id).is_some() || rt.deriveds.get(id).is_some())
}

pub fn get_node_defined_at(_id: NodeId) -> Option<&'static std::panic::Location<'static>> {
    #[cfg(debug_assertions)]
    {
        RUNTIME.with(|rt| {
            if let Some(node) = rt.graph.get(_id) {
                return node.defined_at;
            }
            None
        })
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

// --- Debugging API ---

pub fn set_debug_label(_id: NodeId, _label: impl Into<String>) {
    #[cfg(debug_assertions)]
    {
        let label = _label.into();
        RUNTIME.with(|rt| {
            rt.aux_mut(_id).debug_label = Some(label);
        })
    }
}

pub fn get_debug_label(_id: NodeId) -> Option<String> {
    #[cfg(debug_assertions)]
    {
        return RUNTIME.with(|rt| {
            if let Some(aux) = rt.node_aux.get(_id) {
                if let Some(label) = &aux.debug_label {
                    return Some(label.clone());
                }
            }
            // Check dead labels
            rt.dead_node_labels.get(_id).cloned()
        });
    }
    #[cfg(not(debug_assertions))]
    {
        return None;
    }
}
