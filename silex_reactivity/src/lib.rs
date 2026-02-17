use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

mod arena;
pub use arena::{Arena, Index as NodeId, SparseSecondaryMap};

mod value;
use value::AnyValue;

// --- 基础类型定义 ---

/// 响应式节点通用结构体 (Metadata)。
pub(crate) struct Node {
    pub(crate) children: Vec<NodeId>,
    pub(crate) parent: Option<NodeId>,
    pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
    pub(crate) context: Option<HashMap<TypeId, Box<dyn Any>>>,
    #[cfg(debug_assertions)]
    pub(crate) debug_label: Option<String>,
    #[cfg(debug_assertions)]
    pub(crate) defined_at: Option<&'static std::panic::Location<'static>>,
}

impl Node {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            parent: None,
            cleanups: Vec::new(),
            context: None,
            #[cfg(debug_assertions)]
            debug_label: None,
            #[cfg(debug_assertions)]
            defined_at: None,
        }
    }
}

pub(crate) struct SignalData {
    pub(crate) value: AnyValue,
    pub(crate) subscribers: Vec<NodeId>,
    pub(crate) last_tracked_by: Option<(NodeId, u64)>,
}

pub(crate) struct EffectData {
    pub(crate) computation: Option<Rc<dyn Fn() -> ()>>,
    pub(crate) dependencies: Vec<NodeId>,
    pub(crate) effect_version: u64,
}

/// Callback 数据存储（类型擦除）
pub(crate) struct CallbackData {
    /// 类型擦除的回调函数，接收 Box<dyn Any> 参数
    pub(crate) f: Rc<dyn Fn(Box<dyn Any>)>,
}

/// NodeRef 数据存储（类型擦除）
pub(crate) struct NodeRefData {
    /// 类型擦除的 DOM 节点引用
    pub(crate) element: Option<Box<dyn Any>>,
}

/// StoredValue 数据存储（类型擦除）
pub(crate) struct StoredValueData {
    pub(crate) value: AnyValue,
}

/// Derived 数据存储（类型擦除）
pub(crate) struct DerivedData {
    pub(crate) f: Box<dyn Any>,
}

// --- 响应式系统运行时 ---

pub struct Runtime {
    pub(crate) graph: Arena<Node>,
    pub(crate) signals: SparseSecondaryMap<SignalData>,
    pub(crate) effects: SparseSecondaryMap<EffectData>,
    pub(crate) callbacks: SparseSecondaryMap<CallbackData>,
    pub(crate) node_refs: SparseSecondaryMap<NodeRefData>,
    pub(crate) stored_values: SparseSecondaryMap<StoredValueData>,
    pub(crate) deriveds: SparseSecondaryMap<DerivedData>,

    // Global state
    pub(crate) current_owner: Cell<Option<NodeId>>,
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    pub(crate) queued_observers: SparseSecondaryMap<()>, // Set of queued observers
    pub(crate) running_queue: Cell<bool>,
    pub(crate) batch_depth: Cell<usize>,

    #[cfg(debug_assertions)]
    pub(crate) dead_node_labels: SparseSecondaryMap<String>,
}

thread_local! {
    static RUNTIME: Runtime = Runtime::new();
}

impl Runtime {
    fn new() -> Self {
        Self {
            graph: Arena::new(),
            signals: SparseSecondaryMap::new(),
            effects: SparseSecondaryMap::new(),
            callbacks: SparseSecondaryMap::new(),
            node_refs: SparseSecondaryMap::new(),
            stored_values: SparseSecondaryMap::new(),
            deriveds: SparseSecondaryMap::new(),
            current_owner: Cell::new(None),
            observer_queue: RefCell::new(VecDeque::new()),
            queued_observers: SparseSecondaryMap::new(),
            running_queue: Cell::new(false),
            batch_depth: Cell::new(0),
            #[cfg(debug_assertions)]
            dead_node_labels: SparseSecondaryMap::new(),
        }
    }

    #[track_caller]
    pub(crate) fn register_node(&self) -> NodeId {
        let parent = self.current_owner.get();
        let mut node = Node::new();
        node.parent = parent;

        #[cfg(debug_assertions)]
        {
            node.defined_at = Some(std::panic::Location::caller());
        }

        let id = self.graph.insert(node);

        if let Some(parent_id) = parent {
            if let Some(parent_node) = self.graph.get_mut(parent_id) {
                parent_node.children.push(id);
            }
        }
        id
    }

    #[track_caller]
    pub(crate) fn register_signal_internal<T: 'static>(&self, value: T) -> NodeId {
        let id = self.register_node();
        self.signals.insert(
            id,
            SignalData {
                value: AnyValue::new(value),
                subscribers: Vec::new(),
                last_tracked_by: None,
            },
        );
        id
    }

    #[track_caller]
    pub(crate) fn register_effect_internal<F: Fn() + 'static>(&self, f: F) -> NodeId {
        let id = self.register_node();
        self.effects.insert(
            id,
            EffectData {
                computation: Some(Rc::new(f)),
                dependencies: Vec::new(),
                effect_version: 0,
            },
        );
        id
    }

    pub(crate) fn track_dependency(&self, signal_id: NodeId) {
        if let Some(owner) = self.current_owner.get() {
            if owner == signal_id {
                return;
            }

            if let Some(effect_data) = self.effects.get_mut(owner) {
                if let Some(signal_data) = self.signals.get_mut(signal_id) {
                    let current_version = effect_data.effect_version;
                    if let Some((last_owner, last_version)) = signal_data.last_tracked_by {
                        if last_owner == owner && last_version == current_version {
                            return;
                        }
                    }
                    effect_data.dependencies.push(signal_id);
                    signal_data.subscribers.push(owner);
                    signal_data.last_tracked_by = Some((owner, current_version));
                }
            }
        }
    }

    pub(crate) fn queue_dependents(&self, signal_id: NodeId) {
        // Clone subscribers to avoid holding borrow during iteration
        let subscribers = if let Some(data) = self.signals.get(signal_id) {
            data.subscribers.clone()
        } else {
            Vec::new()
        };

        let mut queue = self.observer_queue.borrow_mut();

        for id in subscribers {
            // Check if already queued
            if self.queued_observers.get(id).is_none() {
                self.queued_observers.insert(id, ());
                queue.push_back(id);
            }
        }
    }

    pub(crate) fn run_queue(&self) {
        if self.running_queue.get() {
            return;
        }
        self.running_queue.set(true);

        loop {
            // Take one from queue
            let next_to_run = self.observer_queue.borrow_mut().pop_front();
            match next_to_run {
                Some(id) => {
                    self.queued_observers.remove(id);
                    run_effect_internal(id);
                }
                None => break,
            }
        }
        self.running_queue.set(false);
    }

    fn clean_node(&self, id: NodeId) {
        let (children, cleanups) = {
            if let Some(node) = self.graph.get_mut(id) {
                (
                    std::mem::take(&mut node.children),
                    std::mem::take(&mut node.cleanups),
                )
            } else {
                return;
            }
        };

        let dependencies = {
            if let Some(effect_data) = self.effects.get_mut(id) {
                std::mem::take(&mut effect_data.dependencies)
            } else {
                Vec::new()
            }
        };

        self.run_cleanups(id, children, cleanups, dependencies);
    }

    fn run_cleanups(
        &self,
        self_id: NodeId,
        children: Vec<NodeId>,
        cleanups: Vec<Box<dyn FnOnce()>>,
        dependencies: Vec<NodeId>,
    ) {
        for cleanup in cleanups {
            cleanup();
        }
        for child in children {
            self.dispose_node_internal(child, false);
        }
        if !dependencies.is_empty() {
            for signal_id in dependencies {
                if let Some(signal_data) = self.signals.get_mut(signal_id) {
                    if let Some(idx) = signal_data.subscribers.iter().position(|&x| x == self_id) {
                        signal_data.subscribers.swap_remove(idx);
                    }
                }
            }
        }
    }

    pub(crate) fn dispose_node_internal(&self, id: NodeId, remove_from_parent: bool) {
        self.clean_node(id);

        #[cfg(debug_assertions)]
        {
            if let Some(node) = self.graph.get_mut(id) {
                if let Some(label) = node.debug_label.take() {
                    self.dead_node_labels.insert(id, label);
                }
            }
        }

        if remove_from_parent {
            if let Some(parent_id) = self.graph.get(id).and_then(|n| n.parent) {
                if let Some(parent_node) = self.graph.get_mut(parent_id) {
                    if let Some(idx) = parent_node.children.iter().position(|&x| x == id) {
                        parent_node.children.swap_remove(idx);
                    }
                }
            }
        }

        self.graph.remove(id);
        self.signals.remove(id);
        self.effects.remove(id);
        self.stored_values.remove(id);
        self.deriveds.remove(id);
        self.queued_observers.remove(id);
        // Note: Can't easily remove from VecDeque efficiently without traversal,
        // but `run_queue` handles spurious IDs gracefully if effect logic checks existence.
        // Actually, our `run_queue` iterates and calls `run_effect_internal`.
        // If node is removed, `run_effect_internal` should check existence.
    }
}

fn run_effect_internal(effect_id: NodeId) {
    RUNTIME.with(|rt| {
        let (children, cleanups) = {
            if let Some(node) = rt.graph.get_mut(effect_id) {
                (
                    std::mem::take(&mut node.children),
                    std::mem::take(&mut node.cleanups),
                )
            } else {
                return;
            }
        };

        let (computation_fn, dependencies) = {
            if let Some(effect_data) = rt.effects.get_mut(effect_id) {
                effect_data.effect_version = effect_data.effect_version.wrapping_add(1);
                (
                    effect_data.computation.clone(),
                    std::mem::take(&mut effect_data.dependencies),
                )
            } else {
                return;
            }
        };

        rt.run_cleanups(effect_id, children, cleanups, dependencies);

        if let Some(f) = computation_fn {
            let prev_owner = rt.current_owner.get();
            rt.current_owner.set(Some(effect_id));
            f();
            rt.current_owner.set(prev_owner);
        }
    })
}

// --- Public High-Level API ---

#[track_caller]
pub fn signal<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| rt.register_signal_internal(value))
}

pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        // Track
        rt.track_dependency(id);

        if let Some(signal) = rt.signals.get(id) {
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
        if let Some(signal) = rt.signals.get(id) {
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
            if let Some(signal) = rt.signals.get_mut(id) {
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
    let id = RUNTIME.with(|rt| rt.register_effect_internal(f));
    run_effect_internal(id);
    id
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
            if let Some(node) = rt.graph.get_mut(owner) {
                node.cleanups.push(Box::new(f));
            }
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
        let effect_id = rt.register_node();

        // Placeholder effect data
        rt.effects.insert(
            effect_id,
            EffectData {
                computation: None,
                dependencies: Vec::new(),
                effect_version: 0,
            },
        );

        // Run once
        let value = {
            let prev_owner = rt.current_owner.get();
            rt.current_owner.set(Some(effect_id));
            let v = f(None);
            rt.current_owner.set(prev_owner);
            v
        };

        // Create inner signal
        let signal_id = rt.register_signal_internal(value);

        // Computation
        let computation = move || {
            // Check old value
            let old_value = RUNTIME.with(|rt| {
                if let Some(signal) = rt.signals.get(signal_id) {
                    if let Some(val) = signal.value.downcast_ref::<T>() {
                        Some(val.clone())
                    } else {
                        None
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
                // Update signal
                update_signal::<T>(signal_id, |v| *v = new_value);
            }
        };

        if let Some(effect_data) = rt.effects.get_mut(effect_id) {
            effect_data.computation = Some(Rc::new(computation));
        }

        signal_id
    })
}

// Context API exposed
pub fn provide_context_any(key: TypeId, value: Box<dyn Any>) {
    RUNTIME.with(|rt| {
        if let Some(owner) = rt.current_owner.get() {
            if let Some(node) = rt.graph.get_mut(owner) {
                if node.context.is_none() {
                    node.context = Some(HashMap::new());
                }
                if let Some(ctx) = &mut node.context {
                    ctx.insert(key, value);
                }
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
            if let Some(node) = rt.graph.get(current) {
                if let Some(ctx) = &node.context {
                    if let Some(val) = ctx.get(&TypeId::of::<T>()) {
                        return val.downcast_ref::<T>().cloned();
                    }
                }
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
        let f_rc: Rc<dyn Fn() -> T> = Rc::new(f);
        rt.deriveds.insert(id, DerivedData { f: Box::new(f_rc) });
        id
    })
}

pub fn run_derived<T: 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        if let Some(data) = rt.deriveds.get(id) {
            if let Some(f) = data.f.downcast_ref::<Rc<dyn Fn() -> T>>() {
                return Some(f());
            }
        }
        None
    })
}

pub fn try_with_signal<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        // Track
        rt.track_dependency(id);

        if let Some(signal) = rt.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

pub fn try_with_signal_untracked<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        if let Some(signal) = rt.signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
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
    RUNTIME.with(|rt| rt.signals.get(id).is_some())
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
            if let Some(node) = rt.graph.get_mut(_id) {
                node.debug_label = Some(label);
            }
        })
    }
}

pub fn get_debug_label(_id: NodeId) -> Option<String> {
    #[cfg(debug_assertions)]
    {
        return RUNTIME.with(|rt| {
            if let Some(node) = rt.graph.get(_id) {
                if let Some(label) = &node.debug_label {
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
