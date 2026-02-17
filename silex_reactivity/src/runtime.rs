use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::DependencyList;
use crate::NodeList;
use crate::algorithm::{self, NodeState, ReactiveGraph};
use crate::arena::{Arena, Index as NodeId, SparseSecondaryMap};
use crate::value::AnyValue;

// --- 基础类型定义 ---

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
    pub(crate) version: u32,
}

pub(crate) struct EffectData {
    pub(crate) computation: Option<Box<dyn Fn()>>,
    pub(crate) dependencies: DependencyList,
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

pub(crate) struct WorkSpace {
    pub(crate) vec_pool: Vec<Vec<NodeId>>,
    pub(crate) deque_pool: Vec<VecDeque<NodeId>>,
}

impl WorkSpace {
    fn new() -> Self {
        Self {
            vec_pool: Vec::new(),
            deque_pool: Vec::new(),
        }
    }

    fn borrow_vec(&mut self) -> Vec<NodeId> {
        self.vec_pool.pop().unwrap_or_default()
    }

    fn return_vec(&mut self, mut v: Vec<NodeId>) {
        v.clear();
        if self.vec_pool.len() < 32 {
            self.vec_pool.push(v);
        }
    }

    fn borrow_deque(&mut self) -> VecDeque<NodeId> {
        self.deque_pool.pop().unwrap_or_default()
    }

    fn return_deque(&mut self, mut d: VecDeque<NodeId>) {
        d.clear();
        if self.deque_pool.len() < 32 {
            self.deque_pool.push(d);
        }
    }
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

    // WorkSpace for reuse
    pub(crate) workspace: RefCell<WorkSpace>,

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
            workspace: RefCell::new(WorkSpace::new()),
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
                version: 0,
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
                dependencies: DependencyList::default(),
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

            // 1. Identify Owner Type and get metadata
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
            let mut target_version = 0;

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
                target_version = signal_data.version;
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
                target_version = derived_data.signal.version;
            }

            if registered {
                // Add to owner's dependency list
                if let Some(eff) = self.effects.get_mut(owner) {
                    eff.dependencies.push((target_id, target_version));
                } else if let Some(der) = self.deriveds.get_mut(owner) {
                    der.effect.dependencies.push((target_id, target_version));
                }
            }
        }
    }

    /// Use BFS Propagation via algorithm module
    pub(crate) fn queue_dependents(&self, source_id: NodeId) {
        let (mut queue, mut subs) = {
            let mut ws = self.workspace.borrow_mut();
            (ws.borrow_deque(), ws.borrow_vec())
        };

        let mut adapter = RuntimeAdapter(self);
        algorithm::propagate(&mut adapter, source_id, &mut queue, &mut subs);

        let mut ws = self.workspace.borrow_mut();
        ws.return_deque(queue);
        ws.return_vec(subs);
    }

    /// Use Iterative DFS (Trampoline) via algorithm module
    pub(crate) fn update_if_necessary(&self, node_id: NodeId) {
        let (mut stack, mut deps) = {
            let mut ws = self.workspace.borrow_mut();
            (ws.borrow_vec(), ws.borrow_vec())
        };

        let mut adapter = RuntimeAdapter(self);
        algorithm::evaluate(&mut adapter, node_id, &mut stack, &mut deps);

        let mut ws = self.workspace.borrow_mut();
        ws.return_vec(stack);
        ws.return_vec(deps);
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
                        // Should primarily use Lazy, but if queued explicitly
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
                let mut deps = DependencyList::default();
                std::mem::swap(&mut effect_data.dependencies, &mut deps);
                deps
            } else if let Some(derived_data) = self.deriveds.get_mut(id) {
                let mut deps = DependencyList::default();
                std::mem::swap(&mut derived_data.effect.dependencies, &mut deps);
                deps
            } else {
                DependencyList::default()
            }
        };

        self.run_cleanups(id, children, cleanups, dependencies);
    }

    pub(crate) fn run_cleanups(
        &self,
        self_id: NodeId,
        children: Vec<NodeId>,
        cleanups: CleanupList,
        dependencies: DependencyList,
    ) {
        for cleanup in cleanups {
            cleanup();
        }
        for child in children {
            self.dispose_node_internal(child, false);
        }
        // Iterate dependencies
        for (dep_id, _) in dependencies {
            if let Some(signal_data) = self.signals.get_mut(dep_id) {
                signal_data.subscribers.remove(&self_id);
            } else if let Some(derived_data) = self.deriveds.get_mut(dep_id) {
                derived_data.signal.subscribers.remove(&self_id);
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
            let mut deps = DependencyList::default();
            std::mem::swap(&mut effect_data.dependencies, &mut deps);
            (effect_data.computation.take(), deps)
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

pub(crate) fn run_derived_internal(rt: &Runtime, derived_id: NodeId) -> bool {
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
            let mut deps = DependencyList::default();
            std::mem::swap(&mut data.effect.dependencies, &mut deps);
            (data.effect.computation.take(), deps)
        } else {
            return false;
        }
    };

    rt.run_cleanups(derived_id, children, cleanups, dependencies);

    // The computation closure (constructed by `memo`) is responsible for:
    // 1. Determining if the new value differs from the old value.
    // 2. Updating the signal value if changed.
    // 3. Queueing dependents if changed.
    // 4. Returning true to indicate a change occurred.
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
        return true;
    }
    false
}

// --- RuntimeAdapter for Algorithm ---

struct RuntimeAdapter<'a>(&'a Runtime);

impl<'a> ReactiveGraph for RuntimeAdapter<'a> {
    fn get_state(&self, id: NodeId) -> NodeState {
        if let Some(d) = self.0.deriveds.get(id) {
            d.state
        } else {
            NodeState::Clean
        }
    }

    fn set_state(&mut self, id: NodeId, state: NodeState) {
        if let Some(d) = self.0.deriveds.get_mut(id) {
            d.state = state;
        }
    }

    fn fill_subscribers(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        if let Some(signal) = self.0.signals.get(id) {
            signal.subscribers.for_each(|&n| dest.push(n));
        } else if let Some(derived) = self.0.deriveds.get(id) {
            derived.signal.subscribers.for_each(|&n| dest.push(n));
        }
    }

    fn fill_dependencies(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        let pusher = |(n, _): &(NodeId, u32)| dest.push(*n);

        if let Some(eff) = self.0.effects.get(id) {
            eff.dependencies.for_each(pusher);
        } else if let Some(der) = self.0.deriveds.get(id) {
            der.effect.dependencies.for_each(pusher);
        }
    }

    fn is_effect(&self, id: NodeId) -> bool {
        self.0.effects.get(id).is_some()
    }

    fn queue_effect(&mut self, id: NodeId) {
        if self.0.queued_observers.get(id).is_none() {
            self.0.queued_observers.insert(id, ());
            self.0.observer_queue.borrow_mut().push_back(id);
        }
    }

    fn run_computation(&mut self, id: NodeId) -> bool {
        run_derived_internal(self.0, id)
    }

    fn check_dependencies_changed(&mut self, id: NodeId) -> bool {
        let mut changed = false;

        let check_fn = |deps: &DependencyList| {
            let mut found_change = false;
            deps.for_each(|(dep_id, expected_ver)| {
                if found_change {
                    return;
                }

                let current_ver = if let Some(s) = self.0.signals.get(*dep_id) {
                    s.version
                } else if let Some(d) = self.0.deriveds.get(*dep_id) {
                    d.signal.version
                } else {
                    // Dependency likely disposed
                    found_change = true;
                    return;
                };

                if current_ver != *expected_ver {
                    found_change = true;
                }
            });
            found_change
        };

        if let Some(eff) = self.0.effects.get(id) {
            changed = check_fn(&eff.dependencies);
        } else if let Some(der) = self.0.deriveds.get(id) {
            changed = check_fn(&der.effect.dependencies);
        }

        changed
    }
}
