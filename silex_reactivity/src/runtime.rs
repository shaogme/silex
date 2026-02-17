use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::arena::{Arena, Index as NodeId, SparseSecondaryMap};
use crate::node_list::NodeList;
use crate::value::AnyValue;

// --- 基础类型定义 ---

/// 响应式节点状态
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NodeState {
    Clean,
    Check,
    Dirty,
}

/// 辅助数据结构，存储“冷数据” (Cold Data)
pub(crate) struct NodeAux {
    pub(crate) children: Vec<NodeId>,
    pub(crate) cleanups: CleanupList,
    pub(crate) context: Option<HashMap<TypeId, Box<dyn Any>>>,
    #[cfg(debug_assertions)]
    pub(crate) debug_label: Option<String>,
}

impl Default for NodeAux {
    fn default() -> Self {
        Self {
            children: Vec::new(),
            cleanups: CleanupList::default(),
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

pub(crate) enum CleanupList {
    Empty,
    Single(Box<dyn FnOnce()>),
    Many(Vec<Box<dyn FnOnce()>>),
}

impl Default for CleanupList {
    fn default() -> Self {
        Self::Empty
    }
}

impl CleanupList {
    pub(crate) fn push(&mut self, f: Box<dyn FnOnce()>) {
        if let Self::Many(vec) = self {
            vec.push(f);
            return;
        }

        let old = std::mem::take(self);
        match old {
            Self::Empty => *self = Self::Single(f),
            Self::Single(prev) => *self = Self::Many(vec![prev, f]),
            Self::Many(_) => unreachable!(),
        }
    }
}

impl IntoIterator for CleanupList {
    type Item = Box<dyn FnOnce()>;
    type IntoIter = CleanupListIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            CleanupList::Empty => CleanupListIntoIter::Empty,
            CleanupList::Single(f) => CleanupListIntoIter::Single(Some(f)),
            CleanupList::Many(vec) => CleanupListIntoIter::Many(vec.into_iter()),
        }
    }
}

pub(crate) enum CleanupListIntoIter {
    Empty,
    Single(Option<Box<dyn FnOnce()>>),
    Many(std::vec::IntoIter<Box<dyn FnOnce()>>),
}

impl Iterator for CleanupListIntoIter {
    type Item = Box<dyn FnOnce()>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Single(opt) => opt.take(),
            Self::Many(iter) => iter.next(),
        }
    }
}

pub(crate) struct SignalData {
    pub(crate) value: AnyValue,
    pub(crate) subscribers: NodeList,
    pub(crate) last_tracked_by: Option<(NodeId, u32)>,
}

pub(crate) struct EffectData {
    pub(crate) computation: Option<Box<dyn Fn()>>,
    pub(crate) dependencies: NodeList,
    pub(crate) effect_version: u32,
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
/// Combined Signal (Value + Subscribers) and Effect (Computation + Dependencies)
pub(crate) struct DerivedData {
    pub(crate) signal: SignalData,
    pub(crate) effect: EffectData,
    pub(crate) state: NodeState,
}

// --- 响应式系统运行时 ---

pub struct Runtime {
    pub(crate) graph: Arena<Node>,
    pub(crate) node_aux: SparseSecondaryMap<NodeAux, 32>,
    pub(crate) signals: SparseSecondaryMap<SignalData, 64>,
    pub(crate) effects: SparseSecondaryMap<EffectData, 64>,
    pub(crate) callbacks: SparseSecondaryMap<CallbackData>, // default 16
    pub(crate) node_refs: SparseSecondaryMap<NodeRefData>,  // default 16
    pub(crate) stored_values: SparseSecondaryMap<StoredValueData>, // default 16
    pub(crate) deriveds: SparseSecondaryMap<DerivedData, 64>,

    // Global state
    pub(crate) current_owner: Cell<Option<NodeId>>,
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    pub(crate) queued_observers: SparseSecondaryMap<()>, // Set of queued observers, default 16 is fine
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
                subscribers: NodeList::Empty,
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
                computation: Some(Box::new(f)),
                dependencies: NodeList::Empty,
                effect_version: 0,
            },
        );
        id
    }

    pub(crate) fn track_dependency(&self, target_id: NodeId) {
        if let Some(owner) = self.current_owner.get() {
            if owner == target_id {
                return;
            }

            // 1. Identify Owner Type (Effect or Derived) and get its metadata
            //    We need to update owner's dependencies list.
            let (owner_version, is_owner_valid) = if let Some(eff) = self.effects.get_mut(owner) {
                (eff.effect_version, true)
            } else if let Some(der) = self.deriveds.get_mut(owner) {
                (der.effect.effect_version, true)
            } else {
                (0, false)
            };

            if !is_owner_valid {
                return;
            }

            // 2. Identify Target Type (Signal or Derived) and register subscription
            let mut registered = false;

            // Check Signal
            if let Some(signal_data) = self.signals.get_mut(target_id) {
                if let Some((last_owner, last_version)) = signal_data.last_tracked_by {
                    if last_owner == owner && last_version == owner_version {
                        return; // Already tracked in this version
                    }
                }
                signal_data.subscribers.push(owner);
                signal_data.last_tracked_by = Some((owner, owner_version));
                registered = true;
            }
            // Check Derived (if not Signal)
            else if let Some(derived_data) = self.deriveds.get_mut(target_id) {
                if let Some((last_owner, last_version)) = derived_data.signal.last_tracked_by {
                    if last_owner == owner && last_version == owner_version {
                        return; // Already tracked in this version
                    }
                }
                derived_data.signal.subscribers.push(owner);
                derived_data.signal.last_tracked_by = Some((owner, owner_version));
                registered = true;
            }

            if registered {
                // Add to owner's dependency list
                if let Some(eff) = self.effects.get_mut(owner) {
                    eff.dependencies.push(target_id);
                } else if let Some(der) = self.deriveds.get_mut(owner) {
                    der.effect.dependencies.push(target_id);
                }
            }
        }
    }

    /// BFS Propagation: Mark direct subscribers of the source as Dirty, and downstream as Check.
    pub(crate) fn queue_dependents(&self, source_id: NodeId) {
        let mut queue = VecDeque::new();
        let mut effects_to_run = Vec::new();

        // 1. Mark direct subscribers as DIRTY
        let direct_subs = if let Some(data) = self.signals.get(source_id) {
            data.subscribers.clone()
        } else if let Some(data) = self.deriveds.get(source_id) {
            data.signal.subscribers.clone()
        } else {
            NodeList::Empty
        };

        for sub_id in direct_subs {
            if let Some(derived) = self.deriveds.get_mut(sub_id) {
                if derived.state != NodeState::Dirty {
                    derived.state = NodeState::Dirty;
                    queue.push_back(sub_id);
                }
            } else if self.effects.get(sub_id).is_some() {
                if self.queued_observers.get(sub_id).is_none() {
                    self.queued_observers.insert(sub_id, ());
                    effects_to_run.push(sub_id);
                }
            }
        }

        // 2. Propagate CHECK to downstream
        while let Some(current_id) = queue.pop_front() {
            let subs = if let Some(data) = self.deriveds.get(current_id) {
                data.signal.subscribers.clone()
            } else {
                NodeList::Empty
            };

            for sub_id in subs {
                if let Some(derived) = self.deriveds.get_mut(sub_id) {
                    // Only propagate if state changes from Clean -> Check
                    if derived.state == NodeState::Clean {
                        derived.state = NodeState::Check;
                        queue.push_back(sub_id);
                    }
                    // If already Check or Dirty, no need to push (already queued or visited)
                } else if self.effects.get(sub_id).is_some() {
                    // Effects depending on Check/Dirty nodes must run
                    if self.queued_observers.get(sub_id).is_none() {
                        self.queued_observers.insert(sub_id, ());
                        effects_to_run.push(sub_id);
                    }
                }
            }
        }

        // 3. Schedule effects
        let mut global_queue = self.observer_queue.borrow_mut();
        for eff_id in effects_to_run {
            global_queue.push_back(eff_id);
        }
    }

    /// Iterative DFS (Trampoline) to validate and update derived nodes.
    /// Prevents stack overflow for deep dependency chains.
    pub(crate) fn update_if_necessary(&self, node_id: NodeId) {
        // Quick check
        if let Some(d) = self.deriveds.get(node_id) {
            if d.state == NodeState::Clean {
                return;
            }
        } else {
            return;
        }

        let mut stack = Vec::with_capacity(16);
        stack.push(node_id);

        while let Some(current) = stack.last().copied() {
            // Peek current node state
            let (state, dependencies) = if let Some(d) = self.deriveds.get(current) {
                // If Clean, we are done with this node, pop it.
                if d.state == NodeState::Clean {
                    stack.pop();
                    continue;
                }
                (d.state, d.effect.dependencies.clone())
            } else {
                // Not a derived node (Signal?), treat as Clean/Done
                stack.pop();
                continue;
            };

            // Step A: Check dependencies for non-Clean states
            let mut found_non_clean_dep = false;
            for dep_id in dependencies {
                if let Some(dep_derived) = self.deriveds.get(dep_id) {
                    if dep_derived.state != NodeState::Clean {
                        stack.push(dep_id);
                        found_non_clean_dep = true;
                        break; // Process dependency first (DFS)
                    }
                }
                // Signals are always "Clean" (sources)
            }

            if found_non_clean_dep {
                continue; // Loop again to process the pushed dependency
            }

            // Step B: All dependencies are Clean. Now we can update `current`.
            if state == NodeState::Check {
                // Optimization: Currently treating Check as Dirty because we lack fine-grained versioning for Signals/Deriveds
                // to robustly skip computation.
                // In a full implementation, we would compare `dep.ptr` or `dep.version`.
                run_derived_internal(self, current);
            } else if state == NodeState::Dirty {
                run_derived_internal(self, current);
            } else {
                // Should be unreachable if logic is correct (Clean handled above)
            }
            // run_derived_internal sets state to Clean.

            // Loop continues, will peek `current` again, find it Clean, and pop.
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
                    // Determine if it is Effect or Derived
                    if self.effects.get(id).is_some() {
                        run_effect_internal(self, id);
                    } else if self.deriveds.get(id).is_some() {
                        // Should primarily use Lazy, but if queued explicitly (e.g. initial setup)
                        self.update_if_necessary(id);
                    }
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
                (Vec::new(), CleanupList::default())
            }
        };

        let dependencies = {
            if let Some(effect_data) = self.effects.get_mut(id) {
                std::mem::take(&mut effect_data.dependencies)
            } else if let Some(derived_data) = self.deriveds.get_mut(id) {
                std::mem::take(&mut derived_data.effect.dependencies)
            } else {
                NodeList::default()
            }
        };

        self.run_cleanups(id, children, cleanups, dependencies);
    }

    pub(crate) fn run_cleanups(
        &self,
        self_id: NodeId,
        children: Vec<NodeId>,
        cleanups: CleanupList,
        dependencies: NodeList,
    ) {
        for cleanup in cleanups {
            cleanup();
        }
        for child in children {
            self.dispose_node_internal(child, false);
        }
        // Iterate dependencies (NodeList)
        // Since we consumed dependencies into the iterator, check if it has items by iterating
        for dep_id in dependencies {
            if let Some(signal_data) = self.signals.get_mut(dep_id) {
                signal_data.subscribers.remove(self_id);
            } else if let Some(derived_data) = self.deriveds.get_mut(dep_id) {
                derived_data.signal.subscribers.remove(self_id);
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

pub(crate) fn run_effect_internal(rt: &Runtime, effect_id: NodeId) {
    let (children, cleanups) = {
        if let Some(aux) = rt.node_aux.get_mut(effect_id) {
            (
                std::mem::take(&mut aux.children),
                std::mem::take(&mut aux.cleanups),
            )
        } else {
            (Vec::new(), CleanupList::default())
        }
    };

    let (computation_fn, dependencies) = {
        if let Some(effect_data) = rt.effects.get_mut(effect_id) {
            effect_data.effect_version = effect_data.effect_version.wrapping_add(1);
            (
                effect_data.computation.take(),
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

        // Put computation back
        if let Some(effect_data) = rt.effects.get_mut(effect_id) {
            effect_data.computation = Some(f);
        }
    }
}

pub(crate) fn run_derived_internal(rt: &Runtime, derived_id: NodeId) {
    let (children, cleanups) = {
        if let Some(aux) = rt.node_aux.get_mut(derived_id) {
            (
                std::mem::take(&mut aux.children),
                std::mem::take(&mut aux.cleanups),
            )
        } else {
            (Vec::new(), CleanupList::default())
        }
    };

    let (computation_fn, dependencies) = {
        if let Some(data) = rt.deriveds.get_mut(derived_id) {
            data.effect.effect_version = data.effect.effect_version.wrapping_add(1);
            (
                data.effect.computation.take(),
                std::mem::take(&mut data.effect.dependencies),
            )
        } else {
            return;
        }
    };

    rt.run_cleanups(derived_id, children, cleanups, dependencies);

    if let Some(f) = computation_fn {
        let prev_owner = rt.current_owner.get();
        rt.current_owner.set(Some(derived_id));
        f();
        rt.current_owner.set(prev_owner);

        // Put computation back and mark Clean
        if let Some(data) = rt.deriveds.get_mut(derived_id) {
            data.effect.computation = Some(f);
            data.state = NodeState::Clean;
        }
    }
}
