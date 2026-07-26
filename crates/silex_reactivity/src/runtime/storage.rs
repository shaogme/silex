use crate::{
    DependencyList, NodeList,
    core::{
        algorithm::{GraphStorage, NodeState},
        arena::{Arena, Index as NodeId, SparseSecondaryMap},
        value::{AnyValue, OnceThunk, ThunkValue},
    },
};
#[cfg(debug_assertions)]
use std::cell::Cell;
use std::{mem, vec::IntoIter};

pub(crate) struct ReactiveNode {
    pub(crate) state: NodeState,
    pub(crate) signal: Option<SignalData>,
    pub(crate) effect: Option<EffectData>,
}

/// 非响应式节点（stored value / callback / node-ref）的载荷。
///
/// 这里曾经是一个五变体的枚举 `ExtraData { Callback, NodeRef, StoredValue,
/// Closure, Op }`，五套几乎一模一样的“取出—downcast—用”的代码，外加五个
/// `is_*_valid` 探测函数（审计报告 §3.1 / §3.2）。变体的**唯一**作用是当运行时的
/// 种类 tag —— 而种类现在写在句柄的类型里（[`crate::Handle`]），这个 tag 就是
/// 纯粹的重复。
///
/// 于是全部收敛成一个 [`AnyValue`]：它自带 SOO、`TypeId` 检查与正确的析构。
/// 连带解决的问题：
///
/// - `ExtraData::Op(RawOpBuffer)` 是 `[MaybeUninit<u8>; 64] + Copy`，节点销毁时
///   只是丢掉 64 字节原始内存，**载荷的析构函数永远不会运行**（§2.4）；
/// - `ExtraData::Closure(Box<dyn Any>)` 装的是一个 `Box<dyn Fn() -> T>`，
///   也就是**双重装箱**，读的时候还要多一次 `Box` 解引用。
pub(crate) struct Payload {
    pub(crate) value: AnyValue,
    /// 值当前是否被借出给某个用户闭包（此时 `value` 是占位值）。
    /// 见 [`crate::runtime::guard::PayloadGuard`]。
    pub(crate) borrowed: bool,
}

impl Payload {
    pub(crate) fn new(value: AnyValue) -> Self {
        Self {
            value,
            borrowed: false,
        }
    }
}

/// 最多为多少个已销毁节点保留调试标签。
///
/// 这张表只增不减，长跑的应用会一直往里堆（AUDIT P14）。它纯粹是为了让
/// “读一个已销毁节点”的报错能说出这个节点原来叫什么，超过上限之后新的标签
/// 直接不记，报错退化成节点编号 —— 比无界增长划算。
#[cfg(debug_assertions)]
pub(crate) const MAX_DEAD_NODE_LABELS: usize = 1024;

pub(crate) struct Storage {
    pub(crate) graph: Arena<Node>,
    pub(crate) node_aux: SparseSecondaryMap<NodeAux, 32>,
    pub(crate) reactive: SparseSecondaryMap<ReactiveNode, 64>,
    pub(crate) extras: SparseSecondaryMap<Payload, 32>,

    #[cfg(debug_assertions)]
    pub(crate) dead_node_labels: SparseSecondaryMap<String>,
    /// 已记下的墓碑标签数量的上界（同一个槽位被覆盖时会多算，只用于封顶）。
    #[cfg(debug_assertions)]
    dead_label_count: Cell<usize>,
}

impl Storage {
    pub(crate) fn new() -> Self {
        Self {
            graph: Arena::new(),
            node_aux: SparseSecondaryMap::new(),
            reactive: SparseSecondaryMap::new(),
            extras: SparseSecondaryMap::new(),
            #[cfg(debug_assertions)]
            dead_node_labels: SparseSecondaryMap::new(),
            #[cfg(debug_assertions)]
            dead_label_count: Cell::new(0),
        }
    }

    /// 为一个即将被销毁的节点留一个墓碑标签，数量封顶（见 [`MAX_DEAD_NODE_LABELS`]）。
    #[cfg(debug_assertions)]
    pub(crate) fn remember_dead_label(&self, id: NodeId, label: String) {
        let count = self.dead_label_count.get();
        if count >= MAX_DEAD_NODE_LABELS {
            return;
        }
        self.dead_label_count.set(count + 1);
        self.dead_node_labels.insert(id, label);
    }

    /// 在闭包作用域内可变地访问一个节点的冷数据，必要时先建出来。
    ///
    /// 节点不在 `graph` 里（已销毁 / 伪造的句柄）时返回 `None` 且不建任何条目。
    pub(crate) fn with_aux_mut<R>(
        &self,
        id: NodeId,
        f: impl FnOnce(&mut NodeAux) -> R,
    ) -> Option<R> {
        if !self.node_aux.contains_key(id) {
            self.graph.get(id)?;
            self.node_aux.insert(id, NodeAux::default());
        }
        self.node_aux.with_mut(id, f)
    }
}

impl GraphStorage for Storage {
    fn get_state(&self, id: NodeId) -> NodeState {
        self.reactive
            .get(id)
            .map(|n| n.state)
            .unwrap_or(NodeState::Clean)
    }

    /// 只更新已存在的节点。
    ///
    /// 之前这里会为不存在的节点**插入**一个空的 `ReactiveNode`：订阅者表里只要
    /// 残留了一个已销毁的 id（`propagate` 遍历时就会遇到），就会为它造出一个
    /// 既不在 `graph` 里、也不会被任何 dispose 路径清理的幽灵条目 —— 长跑的
    /// 应用会一直泄漏下去（AUDIT P14）。
    ///
    /// 忽略掉是安全的：`get_state` 对不存在的节点返回 `Clean`，
    /// 传播与求值都会把它当成“无需处理”。
    fn set_state(&self, id: NodeId, state: NodeState) {
        self.reactive.with_mut(id, |n| n.state = state);
    }

    fn fill_subscribers(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        if let Some(n) = self.reactive.get(id)
            && let Some(signal) = &n.signal
        {
            signal.subscribers.for_each(|&n| dest.push(n));
        }
    }

    fn fill_dependencies(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        if let Some(n) = self.reactive.get(id)
            && let Some(eff) = &n.effect
        {
            eff.dependencies.for_each(|(n, _)| dest.push(*n));
        }
    }

    fn is_effect(&self, id: NodeId) -> bool {
        self.reactive
            .get(id)
            .is_some_and(|n| n.effect.is_some() && n.signal.is_none())
    }

    fn is_running(&self, id: NodeId) -> bool {
        self.reactive
            .get(id)
            .and_then(|n| n.effect.as_ref())
            .is_some_and(|eff| eff.running)
    }

    fn describe(&self, id: NodeId) -> String {
        // release 构建下既没有调试标签也没有定义位置，只剩下编号。
        #[allow(unused_mut)]
        let mut out = format!("节点 #{}", id.slot());
        #[cfg(debug_assertions)]
        {
            if let Some(label) = self
                .node_aux
                .get(id)
                .and_then(|aux| aux.debug_label.as_ref())
            {
                out.push_str(&format!(" “{label}”"));
            }
            if let Some(at) = self.graph.get(id).and_then(|n| n.defined_at) {
                out.push_str(&format!("（定义于 {}:{}）", at.file(), at.line()));
            }
        }
        out
    }

    fn check_dependencies_changed(&self, id: NodeId) -> bool {
        if let Some(n) = self.reactive.get(id)
            && let Some(eff) = &n.effect
        {
            let mut found_change = false;
            eff.dependencies.for_each(|(dep_id, expected_ver)| {
                if found_change {
                    return;
                }
                if let Some(dep_node) = self.reactive.get(*dep_id)
                    && let Some(s) = &dep_node.signal
                {
                    if s.version != *expected_ver {
                        found_change = true;
                    }
                } else {
                    found_change = true;
                }
            });
            found_change
        } else {
            false
        }
    }
}

/// 辅助数据结构，存储“冷数据” (Cold Data)
#[derive(Default)]
pub(crate) struct NodeAux {
    pub(crate) children: Vec<NodeId>,
    pub(crate) cleanups: CleanupList,
    #[cfg(debug_assertions)]
    pub(crate) debug_label: Option<String>,
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

#[derive(Default)]
pub(crate) enum CleanupList {
    #[default]
    Empty,
    Single(OnceThunk),
    Many(Vec<OnceThunk>),
}

impl CleanupList {
    pub(crate) fn push(&mut self, f: OnceThunk) {
        if let Self::Many(vec) = self {
            vec.push(f);
            return;
        }

        let old = mem::take(self);
        match old {
            Self::Empty => *self = Self::Single(f),
            Self::Single(prev) => *self = Self::Many(vec![prev, f]),
            Self::Many(_) => unreachable!("CleanupList::push: impossible state"),
        }
    }
}

impl IntoIterator for CleanupList {
    type Item = OnceThunk;
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
    Single(Option<OnceThunk>),
    Many(IntoIter<OnceThunk>),
}

impl Iterator for CleanupListIntoIter {
    type Item = OnceThunk;
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
    /// 值当前是否被借出给某个 update 闭包（此时 `value` 是占位值）。
    /// 见 [`crate::runtime::guard::SignalValueGuard`]。
    pub(crate) updating: bool,
}

pub(crate) struct EffectData {
    pub(crate) computation: Option<ThunkValue>,
    pub(crate) dependencies: DependencyList,
    pub(crate) effect_version: u32,
    /// 该节点的计算是否正在执行中。
    ///
    /// 重入守卫：正在运行的节点绝不能被重复执行 —— 否则会第二次执行破坏性的
    /// 前置阶段（清空依赖列表、提前跑 cleanup、把自己从所有依赖的订阅者表里摘除），
    /// 而重建订阅的那一步却因为 `computation` 已被借出而被跳过，
    /// 结果是该节点永久丢失全部订阅（AUDIT P1）。
    pub(crate) running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::value::ThunkValue, runtime::Runtime};

    #[test]
    fn set_state_never_inserts_a_ghost_node() {
        let storage = Storage::new();
        let id = storage.graph.insert(Node::new());

        // 这个节点还没有 `reactive` 条目。
        storage.set_state(id, NodeState::Dirty);

        assert!(
            storage.reactive.get(id).is_none(),
            "不得为不存在的节点插入永远不会被回收的幽灵条目（AUDIT P14）"
        );
        assert_eq!(storage.get_state(id), NodeState::Clean);
    }

    /// 订阅者表里残留一个已销毁的 id 时，传播不得为它造出条目。
    #[test]
    fn propagating_to_a_disposed_subscriber_leaves_nothing_behind() {
        let rt = Runtime::new();
        let s = rt.create_signal(AnyValue::new(1i32));

        let dead = rt.create_effect(ThunkValue::new_mut(|| {}));
        rt.dispose(dead);
        assert!(rt.storage.reactive.get(dead).is_none());

        // 手工模拟“订阅者表里残留了一个已销毁的 id”。
        rt.storage.reactive.with_mut(s, |node| {
            if let Some(signal) = node.signal.as_mut() {
                signal.subscribers.push(dead);
            }
        });

        rt.notify_update(s);

        assert!(
            rt.storage.reactive.get(dead).is_none(),
            "传播到已销毁的订阅者时不得复活它（AUDIT P14）"
        );
    }

    /// 墓碑标签只在 debug 构建下存在。
    #[cfg(debug_assertions)]
    #[test]
    fn dead_node_labels_are_capped() {
        let rt = Runtime::new();
        for i in 0..(MAX_DEAD_NODE_LABELS + 16) {
            let id = rt.create_signal(AnyValue::new(i));
            rt.storage
                .with_aux_mut(id, |aux| aux.debug_label = Some(format!("node-{i}")));
            rt.dispose(id);
        }
        assert_eq!(rt.storage.dead_label_count.get(), MAX_DEAD_NODE_LABELS);
    }
}
