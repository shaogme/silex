use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::arena::{Arena, Index as NodeId, SparseSecondaryMap};
use crate::value::AnyValue;

// --- 基础类型定义 ---

/// 辅助数据结构，存储“冷数据” (Cold Data)
pub(crate) struct NodeAux {
    pub(crate) children: Vec<NodeId>,
    pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
    pub(crate) context: Option<HashMap<TypeId, Box<dyn Any>>>,
    #[cfg(debug_assertions)]
    pub(crate) debug_label: Option<String>,
}

impl Default for NodeAux {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            cleanups: Vec::new(),
            context: None,
            #[cfg(debug_assertions)]
            debug_label: None,
        }
    }
}

/// 响应式节点通用结构体 (Metadata)。
/// 仅保留最核心的“热数据”以减小体积。
pub(crate) struct Node {
    pub(crate) parent: Option<NodeId>,
    #[cfg(debug_assertions)]
    pub(crate) defined_at: Option<&'static std::panic::Location<'static>>,
}

impl Node {
    pub(crate) fn new() -> Self {
        Self {
            parent: None,
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
    pub(crate) node_aux: SparseSecondaryMap<NodeAux>,
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
    pub(crate) static RUNTIME: Runtime = Runtime::new();
}

impl Runtime {
    fn new() -> Self {
        Self {
            graph: Arena::new(),
            node_aux: SparseSecondaryMap::new(),
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

    pub(crate) fn aux_mut(&self, id: NodeId) -> &mut NodeAux {
        if self.node_aux.get(id).is_none() {
            self.node_aux.insert(id, NodeAux::default());
        }
        self.node_aux.get_mut(id).unwrap()
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
            self.aux_mut(parent_id).children.push(id);
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

    pub(crate) fn clean_node(&self, id: NodeId) {
        let (children, cleanups) = {
            if let Some(aux) = self.node_aux.get_mut(id) {
                (
                    std::mem::take(&mut aux.children),
                    std::mem::take(&mut aux.cleanups),
                )
            } else {
                (Vec::new(), Vec::new())
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

    pub(crate) fn run_cleanups(
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
            if let Some(aux) = self.node_aux.get_mut(id) {
                if let Some(label) = aux.debug_label.take() {
                    self.dead_node_labels.insert(id, label);
                }
            }
        }

        if remove_from_parent {
            if let Some(parent_id) = self.graph.get(id).and_then(|n| n.parent) {
                if let Some(parent_aux) = self.node_aux.get_mut(parent_id) {
                    if let Some(idx) = parent_aux.children.iter().position(|&x| x == id) {
                        parent_aux.children.swap_remove(idx);
                    }
                }
            }
        }

        self.graph.remove(id);
        self.node_aux.remove(id); // Remove aux
        self.signals.remove(id);
        self.effects.remove(id);
        self.stored_values.remove(id);
        self.deriveds.remove(id);
        self.queued_observers.remove(id);
    }
}

pub(crate) fn run_effect_internal(effect_id: NodeId) {
    RUNTIME.with(|rt| {
        let (children, cleanups) = {
            if let Some(aux) = rt.node_aux.get_mut(effect_id) {
                (
                    std::mem::take(&mut aux.children),
                    std::mem::take(&mut aux.cleanups),
                )
            } else {
                (Vec::new(), Vec::new())
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
