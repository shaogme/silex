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
/// 这里只存储所有节点共有的拓扑和生命周期数据。
/// 专用数据（Signal Values, Effect Dependencies）被拆分到 SecondaryMap 中。
pub(crate) struct Node {
    /// 子节点列表 (Scope/Effect -> Recursive)。用于生命周期管理。
    pub(crate) children: Vec<NodeId>,
    /// 父节点 ID。
    pub(crate) parent: Option<NodeId>,
    /// 清理回调函数列表。
    pub(crate) cleanups: Vec<Box<dyn FnOnce()>>,
    /// 上下文存储 (Context)。
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

/// 仅 Signal 节点使用的数据
pub(crate) struct SignalData {
    pub(crate) value: Box<dyn Any>,
    pub(crate) subscribers: Vec<NodeId>,
    /// 记录上一次追踪此 Signal 的 (OwnerId, OwnerVersion)。
    /// 优化：O(1) 依赖查重。
    pub(crate) last_tracked_by: Option<(NodeId, u64)>,
}

/// 仅 Effect 节点使用的数据
pub(crate) struct EffectData {
    pub(crate) computation: Option<Rc<dyn Fn() -> ()>>,
    pub(crate) dependencies: Vec<NodeId>,
    /// 记录当前 Effect 运行的版本号（次数）。
    pub(crate) effect_version: u64,
}

// --- 响应式系统运行时 ---

pub(crate) struct Runtime {
    /// 存储所有活动节点的 SlotMap。
    pub(crate) nodes: RefCell<SlotMap<NodeId, Node>>,
    /// 存储 Signal 专用数据 (Split Store)。
    pub(crate) signals: RefCell<SecondaryMap<NodeId, SignalData>>,
    /// 存储 Effect 专用数据 (Split Store)。
    pub(crate) effects: RefCell<SecondaryMap<NodeId, EffectData>>,
    /// 当前正在运行的 Effect 或 Scope 的 ID (Owner)。
    pub(crate) current_owner: RefCell<Option<NodeId>>,
    /// 待运行的副作用队列 (FIFO)。
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    /// 已经进入队列的副作用集合 (用于去重)。
    pub(crate) queued_observers: RefCell<SecondaryMap<NodeId, ()>>,
    /// 标志：是否正在运行队列 (防止递归重入)。
    pub(crate) running_queue: Cell<bool>,
}

thread_local! {
    /// 线程局部的 Runtime 实例。
    pub(crate) static RUNTIME: Runtime = Runtime::new();
}

impl Runtime {
    fn new() -> Self {
        Self {
            nodes: RefCell::new(SlotMap::with_key()),
            signals: RefCell::new(SecondaryMap::new()),
            effects: RefCell::new(SecondaryMap::new()),
            current_owner: RefCell::new(None),
            observer_queue: RefCell::new(VecDeque::new()),
            queued_observers: RefCell::new(SecondaryMap::new()),
            running_queue: Cell::new(false),
        }
    }

    // --- 核心操作 ---

    /// 注册一个新的节点到运行时系统中。
    pub(crate) fn register_node(&self) -> NodeId {
        let mut nodes = self.nodes.borrow_mut();
        self.register_node_internal(&mut nodes)
    }

    /// 内部辅助函数：在已持有锁的情况下注册节点。
    fn register_node_internal(&self, nodes: &mut SlotMap<NodeId, Node>) -> NodeId {
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

    /// 注册一个新的 Signal。
    pub(crate) fn register_signal<T: 'static>(&self, value: T) -> NodeId {
        let mut nodes = self.nodes.borrow_mut();
        let id = self.register_node_internal(&mut nodes);

        // 初始化 Signal 数据
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

    /// 注册一个新的 Effect。
    /// 这是一个专用辅助函数，用于简化 create_effect。
    pub(crate) fn register_effect<F: Fn() + 'static>(&self, f: F) -> NodeId {
        let mut nodes = self.nodes.borrow_mut();
        let id = self.register_node_internal(&mut nodes);

        // 初始化 Effect 数据
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

    /// 追踪依赖关系。
    /// 当一个 Signal 被读取时调用，将其添加到当前运行的 Effect 的依赖列表中。
    pub(crate) fn track_dependency(&self, signal_id: NodeId) {
        if let Some(owner) = *self.current_owner.borrow() {
            // 显式处理自依赖情况
            if owner == signal_id {
                return;
            }

            // 获取 Effect 数据 (Owner)
            let mut effects = self.effects.borrow_mut();
            if let Some(effect_data) = effects.get_mut(owner) {
                // 获取 Signal 数据
                let mut signals = self.signals.borrow_mut();
                if let Some(signal_data) = signals.get_mut(signal_id) {
                    // 优化：Run Versioning (O(1) 查重)
                    let current_version = effect_data.effect_version;

                    if let Some((last_owner, last_version)) = signal_data.last_tracked_by {
                        if last_owner == owner && last_version == current_version {
                            return; // 已经追踪过
                        }
                    }

                    // 建立双向依赖
                    effect_data.dependencies.push(signal_id);
                    signal_data.subscribers.push(owner);

                    // 更新 Signal 的追踪标记
                    signal_data.last_tracked_by = Some((owner, current_version));
                }
            }
        }
    }

    /// 获取 Signal 的所有依赖者（订阅者）。
    pub(crate) fn get_dependents(&self, signal_id: NodeId) -> Vec<NodeId> {
        let signals = self.signals.borrow();
        if let Some(data) = signals.get(signal_id) {
            data.subscribers.clone()
        } else {
            Vec::new()
        }
    }

    /// 清理节点。
    pub(crate) fn clean_node(&self, id: NodeId) {
        // 1. 获取并移除所有通用资源
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

        // 2. 如果是 Effect，还需要清理依赖关系
        let mut dependencies = Vec::new();
        {
            let mut effects = self.effects.borrow_mut();
            if let Some(effect_data) = effects.get_mut(id) {
                dependencies = std::mem::take(&mut effect_data.dependencies);
            }
        }

        self.run_cleanups(id, children, cleanups, dependencies);
    }

    /// 执行清理逻辑
    fn run_cleanups(
        &self,
        self_id: NodeId,
        children: Vec<NodeId>,
        cleanups: Vec<Box<dyn FnOnce()>>,
        dependencies: Vec<NodeId>,
    ) {
        // 1. 递归销毁子节点 (从 Runtime 移除)
        for child in children {
            self.dispose_node(child, false);
        }

        // 2. 运行清理回调
        for cleanup in cleanups {
            cleanup();
        }

        // 3. 解除依赖关系
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

    /// 销毁节点。
    pub(crate) fn dispose_node(&self, id: NodeId, remove_from_parent: bool) {
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

        // 如果该节点在队列中，也应该移除（可选，但在 lazy cleanup 中很有用）
        // 这里简单处理：运行时检查 effect 是否存在即可，不需要从 queue 线性查找移除
        // 但 queued_observers 是 SecondaryMap，可以移除标记
        if self.queued_observers.borrow().contains_key(id) {
            self.queued_observers.borrow_mut().remove(id);
        }
    }

    /// 将依赖于指定 Signal 的所有副作用加入队列。
    pub(crate) fn queue_dependents(&self, signal_id: NodeId) {
        let dependents = self.get_dependents(signal_id);
        let mut queue = self.observer_queue.borrow_mut();
        let mut queued = self.queued_observers.borrow_mut();

        for id in dependents {
            if !queued.contains_key(id) {
                queued.insert(id, ());
                queue.push_back(id);
            }
        }
    }

    /// 运行任务队列，执行所有挂起的副作用。
    /// 使用 Breadth-First 策略展平调用栈，避免递归溢出和 RefCell 借用冲突。
    pub(crate) fn run_queue(&self) {
        // 防止递归调用：如果已经在运行队列，直接返回
        if self.running_queue.get() {
            return;
        }
        self.running_queue.set(true);

        // 循环直到队列为空
        loop {
            // 1. 取出一个待执行任务
            let next_to_run = {
                // 仅在弹出时持有借用
                self.observer_queue.borrow_mut().pop_front()
            };

            match next_to_run {
                Some(id) => {
                    // 2. 从去重集合移除标记，允许后续再次加入
                    self.queued_observers.borrow_mut().remove(id);

                    // 3. 执行副作用
                    // 注意：这里我们不持有任何 Runtime 的 RefCell 借用
                    run_effect(id);
                }
                None => break, // 队列已空
            }
        }

        self.running_queue.set(false);
    }
}

/// 运行一个 Effect。
pub(crate) fn run_effect(effect_id: NodeId) {
    RUNTIME.with(|rt| {
        // 1. 获取计算闭包和资源
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
                // 增加版本号
                effect_data.effect_version = effect_data.effect_version.wrapping_add(1);
                (
                    effect_data.computation.clone(),
                    std::mem::take(&mut effect_data.dependencies),
                )
            } else {
                return;
            }
        };

        // 2. 清理
        rt.run_cleanups(effect_id, children, cleanups, dependencies);

        // 3. 执行
        if let Some(f) = computation_fn {
            let prev_owner = *rt.current_owner.borrow();
            *rt.current_owner.borrow_mut() = Some(effect_id);

            f();

            *rt.current_owner.borrow_mut() = prev_owner;
        }
    })
}
