use slotmap::{SecondaryMap, SlotMap, new_key_type};
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

// --- 基础类型定义 ---

new_key_type! {
    /// 响应式节点的唯一标识符。
    pub struct NodeId;
}

/// 响应式节点通用结构体 (Metadata)。
pub(crate) struct Node {
    pub(crate) children: Vec<NodeId>,
    pub(crate) parent: Option<NodeId>,
    pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
    pub(crate) context: Option<HashMap<TypeId, Box<dyn Any>>>,
}

impl Node {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            parent: None,
            cleanups: Vec::new(),
            context: None,
        }
    }
}

pub(crate) struct SignalData {
    pub(crate) value: Box<dyn Any>,
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
    pub(crate) value: Box<dyn Any>,
}

/// Derived 数据存储（类型擦除）
pub(crate) struct DerivedData {
    pub(crate) f: Rc<dyn Fn() -> Box<dyn Any>>,
}

// --- 响应式系统运行时 ---

pub struct Runtime {
    pub(crate) nodes: RefCell<SlotMap<NodeId, Node>>,
    pub(crate) signals: RefCell<SecondaryMap<NodeId, SignalData>>,
    pub(crate) effects: RefCell<SecondaryMap<NodeId, EffectData>>,
    pub(crate) callbacks: RefCell<SecondaryMap<NodeId, CallbackData>>,
    pub(crate) node_refs: RefCell<SecondaryMap<NodeId, NodeRefData>>,
    pub(crate) stored_values: RefCell<SecondaryMap<NodeId, StoredValueData>>,
    pub(crate) deriveds: RefCell<SecondaryMap<NodeId, DerivedData>>,
    pub(crate) current_owner: RefCell<Option<NodeId>>,
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    pub(crate) queued_observers: RefCell<SecondaryMap<NodeId, ()>>,
    pub(crate) running_queue: Cell<bool>,
    pub(crate) batch_depth: Cell<usize>,
}

thread_local! {
    static RUNTIME: Runtime = Runtime::new();
}

impl Runtime {
    fn new() -> Self {
        Self {
            nodes: RefCell::new(SlotMap::with_key()),
            signals: RefCell::new(SecondaryMap::new()),
            effects: RefCell::new(SecondaryMap::new()),
            callbacks: RefCell::new(SecondaryMap::new()),
            node_refs: RefCell::new(SecondaryMap::new()),
            stored_values: RefCell::new(SecondaryMap::new()),
            deriveds: RefCell::new(SecondaryMap::new()),
            current_owner: RefCell::new(None),
            observer_queue: RefCell::new(VecDeque::new()),
            queued_observers: RefCell::new(SecondaryMap::new()),
            running_queue: Cell::new(false),
            batch_depth: Cell::new(0),
        }
    }

    pub(crate) fn register_node(&self) -> NodeId {
        let mut nodes = self.nodes.borrow_mut();
        let parent = *self.current_owner.borrow();
        let mut node = Node::new();
        node.parent = parent;

        let id = nodes.insert(node);

        if let Some(parent_id) = parent {
            if let Some(parent_node) = nodes.get_mut(parent_id) {
                parent_node.children.push(id);
            }
        }
        id
    }

    pub(crate) fn register_signal_internal<T: 'static>(&self, value: T) -> NodeId {
        let id = self.register_node();
        self.signals.borrow_mut().insert(
            id,
            SignalData {
                value: Box::new(value),
                subscribers: Vec::new(),
                last_tracked_by: None,
            },
        );
        id
    }

    pub(crate) fn register_effect_internal<F: Fn() + 'static>(&self, f: F) -> NodeId {
        let id = self.register_node();
        self.effects.borrow_mut().insert(
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
        if let Some(owner) = *self.current_owner.borrow() {
            if owner == signal_id {
                return;
            }

            let mut effects = self.effects.borrow_mut();
            if let Some(effect_data) = effects.get_mut(owner) {
                let mut signals = self.signals.borrow_mut();
                if let Some(signal_data) = signals.get_mut(signal_id) {
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
        let signals = self.signals.borrow();
        let subscribers = if let Some(data) = signals.get(signal_id) {
            data.subscribers.clone()
        } else {
            Vec::new()
        };
        drop(signals);

        let mut queue = self.observer_queue.borrow_mut();
        let mut queued = self.queued_observers.borrow_mut();

        for id in subscribers {
            if !queued.contains_key(id) {
                queued.insert(id, ());
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
            let next_to_run = self.observer_queue.borrow_mut().pop_front();
            match next_to_run {
                Some(id) => {
                    self.queued_observers.borrow_mut().remove(id);
                    run_effect_internal(id);
                }
                None => break,
            }
        }
        self.running_queue.set(false);
    }

    fn clean_node(&self, id: NodeId) {
        let (children, cleanups) = {
            let mut nodes = self.nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(id) {
                (
                    std::mem::take(&mut node.children),
                    std::mem::take(&mut node.cleanups),
                )
            } else {
                return;
            }
        };

        let dependencies = {
            let mut effects = self.effects.borrow_mut();
            if let Some(effect_data) = effects.get_mut(id) {
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
        for child in children {
            self.dispose_node_internal(child, false);
        }
        for cleanup in cleanups {
            cleanup();
        }
        if !dependencies.is_empty() {
            let mut signals = self.signals.borrow_mut();
            for signal_id in dependencies {
                if let Some(signal_data) = signals.get_mut(signal_id) {
                    if let Some(idx) = signal_data.subscribers.iter().position(|&x| x == self_id) {
                        signal_data.subscribers.swap_remove(idx);
                    }
                }
            }
        }
    }

    pub(crate) fn dispose_node_internal(&self, id: NodeId, remove_from_parent: bool) {
        self.clean_node(id);

        let mut nodes = self.nodes.borrow_mut();
        if remove_from_parent {
            let parent_id = nodes.get(id).and_then(|n| n.parent);
            if let Some(parent) = parent_id {
                if let Some(parent_node) = nodes.get_mut(parent) {
                    if let Some(idx) = parent_node.children.iter().position(|&x| x == id) {
                        parent_node.children.swap_remove(idx);
                    }
                }
            }
        }
        nodes.remove(id);
        self.signals.borrow_mut().remove(id);
        self.effects.borrow_mut().remove(id);
        self.stored_values.borrow_mut().remove(id);
        self.deriveds.borrow_mut().remove(id);
        if self.queued_observers.borrow().contains_key(id) {
            self.queued_observers.borrow_mut().remove(id);
        }
    }
}

fn run_effect_internal(effect_id: NodeId) {
    RUNTIME.with(|rt| {
        let (children, cleanups) = {
            let mut nodes = rt.nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(effect_id) {
                (
                    std::mem::take(&mut node.children),
                    std::mem::take(&mut node.cleanups),
                )
            } else {
                return;
            }
        };

        let (computation_fn, dependencies) = {
            let mut effects = rt.effects.borrow_mut();
            if let Some(effect_data) = effects.get_mut(effect_id) {
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
            let prev_owner = *rt.current_owner.borrow();
            *rt.current_owner.borrow_mut() = Some(effect_id);
            f();
            *rt.current_owner.borrow_mut() = prev_owner;
        }
    })
}

// --- Public High-Level API ---

pub fn signal<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| rt.register_signal_internal(value))
}

pub fn try_get_signal<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        // Track
        rt.track_dependency(id);

        let signals = rt.signals.borrow();
        if let Some(signal) = signals.get(id) {
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
        let signals = rt.signals.borrow();
        if let Some(signal) = signals.get(id) {
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
            let mut signals = rt.signals.borrow_mut();
            if let Some(signal) = signals.get_mut(id) {
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

pub fn effect<F: Fn() + 'static>(f: F) -> NodeId {
    let id = RUNTIME.with(|rt| rt.register_effect_internal(f));
    run_effect_internal(id);
    id
}

pub fn create_scope<F>(f: F) -> NodeId
where
    F: FnOnce(),
{
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        let prev_owner = *rt.current_owner.borrow();
        *rt.current_owner.borrow_mut() = Some(id);
        f();
        *rt.current_owner.borrow_mut() = prev_owner;
        id
    })
}

pub fn dispose(id: NodeId) {
    RUNTIME.with(|rt| rt.dispose_node_internal(id, true));
}

pub fn on_cleanup(f: impl FnOnce() + 'static) {
    RUNTIME.with(|rt| {
        if let Some(owner) = *rt.current_owner.borrow() {
            let mut nodes = rt.nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(owner) {
                node.cleanups.push(Box::new(f));
            }
        }
    });
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.with(|rt| {
        let prev_owner = *rt.current_owner.borrow();
        *rt.current_owner.borrow_mut() = None;
        let t = f();
        *rt.current_owner.borrow_mut() = prev_owner;
        t
    })
}

// Provide generic memo creation
pub fn memo<T, F>(f: F) -> NodeId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    RUNTIME.with(|rt| {
        let effect_id = rt.register_node();

        // Placeholder effect data
        rt.effects.borrow_mut().insert(
            effect_id,
            EffectData {
                computation: None,
                dependencies: Vec::new(),
                effect_version: 0,
            },
        );

        // Run once
        let value = {
            let prev_owner = *rt.current_owner.borrow();
            *rt.current_owner.borrow_mut() = Some(effect_id);
            let v = f(None);
            *rt.current_owner.borrow_mut() = prev_owner;
            v
        };

        // Create inner signal
        let signal_id = rt.register_signal_internal(value);

        // Computation
        let computation = move || {
            // Check old value
            let old_value = RUNTIME.with(|rt| {
                let signals = rt.signals.borrow();
                if let Some(signal) = signals.get(signal_id) {
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
                // Should technically not happen if initialized, but ...
                changed = true;
            }

            if changed {
                // Update signal
                update_signal::<T>(signal_id, |v| *v = new_value);
            }
        };

        if let Some(effect_data) = rt.effects.borrow_mut().get_mut(effect_id) {
            effect_data.computation = Some(Rc::new(computation));
        }

        signal_id
    })
}

// Context API exposed
pub fn provide_context_any(key: TypeId, value: Box<dyn Any>) {
    RUNTIME.with(|rt| {
        if let Some(owner) = *rt.current_owner.borrow() {
            let mut nodes = rt.nodes.borrow_mut();
            if let Some(node) = nodes.get_mut(owner) {
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
        let nodes = rt.nodes.borrow();
        let mut current_opt = *rt.current_owner.borrow();

        while let Some(current) = current_opt {
            if let Some(node) = nodes.get(current) {
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

/// 注册一个回调函数，返回其 NodeId。
/// 回调函数接收类型擦除的参数 `Box<dyn Any>`。
pub fn register_callback<F>(f: F) -> NodeId
where
    F: Fn(Box<dyn Any>) + 'static,
{
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.callbacks
            .borrow_mut()
            .insert(id, CallbackData { f: Rc::new(f) });
        id
    })
}

/// 调用指定 ID 的回调函数。
/// 如果回调不存在，则静默忽略。
pub fn invoke_callback(id: NodeId, arg: Box<dyn Any>) {
    RUNTIME.with(|rt| {
        let callback = {
            let callbacks = rt.callbacks.borrow();
            callbacks.get(id).map(|data| data.f.clone())
        };
        if let Some(f) = callback {
            f(arg);
        }
    })
}

/// 检查指定 ID 是否为有效的 Callback
pub fn is_callback_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.callbacks.borrow().contains_key(id))
}

// --- NodeRef API ---

/// 注册一个 NodeRef，返回其 NodeId。
/// 初始状态为空（None）。
pub fn register_node_ref() -> NodeId {
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.node_refs
            .borrow_mut()
            .insert(id, NodeRefData { element: None });
        id
    })
}

/// 获取 NodeRef 中存储的元素引用。
/// 需要调用者指定正确的类型 T 进行 downcast。
pub fn get_node_ref<T: Clone + 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        let node_refs = rt.node_refs.borrow();
        if let Some(data) = node_refs.get(id) {
            if let Some(ref element) = data.element {
                return element.downcast_ref::<T>().cloned();
            }
        }
        None
    })
}

/// 设置 NodeRef 中存储的元素引用。
pub fn set_node_ref<T: 'static>(id: NodeId, element: T) {
    RUNTIME.with(|rt| {
        let mut node_refs = rt.node_refs.borrow_mut();
        if let Some(data) = node_refs.get_mut(id) {
            data.element = Some(Box::new(element));
        }
    })
}

/// Check if the specified ID is a valid NodeRef
pub fn is_node_ref_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.node_refs.borrow().contains_key(id))
}

/// Track signal manually
pub fn track_signal(id: NodeId) {
    RUNTIME.with(|rt| rt.track_dependency(id))
}

/// Notify dependent effects
pub fn notify_signal(id: NodeId) {
    RUNTIME.with(|rt| {
        rt.queue_dependents(id);
        if rt.batch_depth.get() == 0 {
            rt.run_queue();
        }
    })
}

// --- StoredValue API ---

pub fn store_value<T: 'static>(value: T) -> NodeId {
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.stored_values.borrow_mut().insert(
            id,
            StoredValueData {
                value: Box::new(value),
            },
        );
        id
    })
}

pub fn try_with_stored_value<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        let stored = rt.stored_values.borrow();
        if let Some(data) = stored.get(id) {
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
        let mut stored = rt.stored_values.borrow_mut();
        if let Some(data) = stored.get_mut(id) {
            if let Some(val) = data.value.downcast_mut::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

// --- Derived API ---

pub fn register_derived<F>(f: F) -> NodeId
where
    F: Fn() -> Box<dyn Any> + 'static,
{
    RUNTIME.with(|rt| {
        let id = rt.register_node();
        rt.deriveds
            .borrow_mut()
            .insert(id, DerivedData { f: Rc::new(f) });
        id
    })
}

pub fn run_derived<T: 'static>(id: NodeId) -> Option<T> {
    RUNTIME.with(|rt| {
        let deriveds = rt.deriveds.borrow();
        if let Some(data) = deriveds.get(id) {
            let res_box = (data.f)();
            if let Ok(val) = res_box.downcast::<T>() {
                return Some(*val);
            }
        }
        None
    })
}

pub fn try_with_signal<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        // Track
        rt.track_dependency(id);

        let signals = rt.signals.borrow();
        if let Some(signal) = signals.get(id) {
            if let Some(val) = signal.value.downcast_ref::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

pub fn try_with_signal_untracked<T: 'static, R>(id: NodeId, f: impl FnOnce(&T) -> R) -> Option<R> {
    RUNTIME.with(|rt| {
        let signals = rt.signals.borrow();
        if let Some(signal) = signals.get(id) {
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
        let mut signals = rt.signals.borrow_mut();
        if let Some(signal) = signals.get_mut(id) {
            if let Some(val) = signal.value.downcast_mut::<T>() {
                return Some(f(val));
            }
        }
        None
    })
}

pub fn is_signal_valid(id: NodeId) -> bool {
    RUNTIME.with(|rt| rt.signals.borrow().contains_key(id))
}
