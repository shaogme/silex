//! 节点的存储表示。
//!
//! # 阶段三：冷热分离
//!
//! 这个 crate 从前建立在“`&self` + `UnsafeCell` 的裸内部可变性”上：独占性只靠
//! 注释维系，而每一个用户回调边界都是一次潜在的别名违规（审计报告 §4 阶段三）。
//! 现在借用检查交回给类型系统与运行时：
//!
//! | 分类 | 存法 | 为什么 |
//! |---|---|---|
//! | **元数据**（状态、版本、标志） | [`Cell`] | 这是 `propagate` / `evaluate` 99% 的访问。`Cell` 只要共享引用就能写，因此“持有一个节点的引用去改另一个节点的状态”是合法的 —— 订阅表与依赖表得以**原地遍历**，`Vec` 物化与配套池化一并消失（§3.3） |
//! | **载荷**（值、闭包、依赖表、订阅表） | [`RefCell`] | 今天静默的 UB 变成一句明确的诊断，而且直接说得出“你在闭包里重入了同一个节点” |
//!
//! 两个“借出中”的布尔标志（`SignalData::updating` / `EffectData::computation`
//! 的 `None` 语义）也随之收敛：值与闭包都用 `Option` 表示借出，
//! 借出期间节点里是 `None` 而不再是一个现造的 `AnyValue::placeholder()`。

use crate::{
    DependencyList, NodeList,
    internal::{
        arena::{Arena, Index as NodeId, SparseSecondaryMap},
        value::{AnyValue, Computation, EffectThunk, MemoThunk, OnceThunk},
    },
    runtime::graph::NodeState,
};
use std::{
    cell::{Cell, RefCell},
    mem,
    vec::IntoIter,
};

/// 一个节点的种类与运行状态，打包进一个字节。
///
/// 这些位从前分散成 `Option<SignalData>` / `Option<EffectData>` 的存在性、
/// 外加 `SignalData::updating` 与 `EffectData::running` 两个 `bool`。判定种类
/// 因此要去看载荷 —— 而载荷现在藏在 `RefCell` 后面，热路径不该为了问一句
/// “这是不是 effect”去借一次。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct NodeFlags(u8);

impl NodeFlags {
    /// 带一个可读的值：signal / memo / derived。
    pub(crate) const VALUE: Self = Self(1 << 0);
    /// 带一个计算闭包：effect / memo / derived。
    pub(crate) const COMPUTATION: Self = Self(1 << 1);
    /// 计算正在执行中。
    ///
    /// 重入守卫：正在运行的节点绝不能被重复执行 —— 否则会第二次执行破坏性的
    /// 前置阶段（清空依赖列表、提前跑 cleanup、把自己从所有依赖的订阅者表里
    /// 摘除），而重建订阅的那一步却因为闭包已被借出而被跳过，结果是该节点
    /// 永久丢失全部订阅（AUDIT P1）。
    pub(crate) const RUNNING: Self = Self(1 << 2);

    #[inline(always)]
    pub(crate) const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline(always)]
    pub(crate) const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    #[inline(always)]
    pub(crate) const fn has(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

/// signal 载荷：值与订阅者表。
///
/// `value == None` 表示值正被某个用户闭包借出（见
/// [`crate::runtime::guard::SignalValueGuard`]）。从前这是
/// `updating: bool` 加一个塞进节点的 `AnyValue::placeholder()`；用 `Option`
/// 表达同一件事，既少一个字段，也省掉每次写入现造一个占位值。
#[derive(Default)]
pub(crate) struct SignalSlot {
    pub(crate) value: Option<AnyValue>,
    pub(crate) subscribers: NodeList,
}

/// 计算载荷：计算闭包与依赖表。
///
/// `computation == None` 表示闭包正被借出（节点正在运行，见
/// [`crate::runtime::guard::NodeRunGuard`]）。
#[derive(Default)]
pub(crate) struct EffectSlot {
    pub(crate) computation: Option<Computation>,
    pub(crate) dependencies: DependencyList,
}

pub(crate) struct ReactiveNode {
    // --- 热元数据：`Cell`，共享引用就能读写，不作废任何别的引用 ---
    pub(crate) state: Cell<NodeState>,
    pub(crate) flags: Cell<NodeFlags>,
    /// signal 值的版本号，只在值**真的变了**时递增（AUDIT P12）。
    pub(crate) version: Cell<u32>,
    /// 本节点作为 observer 的运行代次，用于 `last_tracked_by` 的去重。
    pub(crate) effect_version: Cell<u32>,
    /// 按 signal 存的单条去重缓存：本次运行是否已经把某个 observer 登记过。
    pub(crate) last_tracked_by: Cell<Option<(NodeId, u32)>>,

    // --- 载荷：`RefCell`，重入即诊断 ---
    pub(crate) signal: RefCell<SignalSlot>,
    pub(crate) effect: RefCell<EffectSlot>,
}

impl ReactiveNode {
    pub(crate) fn new(
        state: NodeState,
        flags: NodeFlags,
        signal: SignalSlot,
        effect: EffectSlot,
    ) -> Self {
        Self {
            state: Cell::new(state),
            flags: Cell::new(flags),
            version: Cell::new(0),
            effect_version: Cell::new(0),
            last_tracked_by: Cell::new(None),
            signal: RefCell::new(signal),
            effect: RefCell::new(effect),
        }
    }

    pub(crate) fn new_signal(value: AnyValue) -> Self {
        Self::new(
            NodeState::Clean,
            NodeFlags::VALUE,
            SignalSlot {
                value: Some(value),
                subscribers: NodeList::Empty,
            },
            EffectSlot::default(),
        )
    }

    pub(crate) fn new_effect(computation: EffectThunk) -> Self {
        Self::new(
            NodeState::Clean,
            NodeFlags::COMPUTATION,
            SignalSlot::default(),
            EffectSlot {
                computation: Some(Computation::Effect(computation)),
                dependencies: DependencyList::default(),
            },
        )
    }

    /// memo / derived：既有值又有计算，且从 `Dirty` 起步（首次读取时才算）。
    pub(crate) fn new_memo(computation: MemoThunk) -> Self {
        Self::new(
            NodeState::Dirty,
            NodeFlags::VALUE.with(NodeFlags::COMPUTATION),
            // 首算之前没有值；`None` 同时也是“借出中”的表示，而首算恰好就是
            // 一次“把旧值借出去”的过程，两者天然一致。
            SignalSlot::default(),
            EffectSlot {
                computation: Some(Computation::Memo(computation)),
                dependencies: DependencyList::default(),
            },
        )
    }

    #[inline(always)]
    pub(crate) fn has_value(&self) -> bool {
        self.flags.get().has(NodeFlags::VALUE)
    }

    #[inline(always)]
    pub(crate) fn is_computation(&self) -> bool {
        self.flags.get().has(NodeFlags::COMPUTATION)
    }

    /// 是不是一个**纯**副作用节点（有计算、没有值）——
    /// 也就是传播时该被推进队列、而不是继续往下走的那种。
    #[inline(always)]
    pub(crate) fn is_effect(&self) -> bool {
        let flags = self.flags.get();
        flags.has(NodeFlags::COMPUTATION) && !flags.has(NodeFlags::VALUE)
    }

    #[inline(always)]
    pub(crate) fn is_running(&self) -> bool {
        self.flags.get().has(NodeFlags::RUNNING)
    }

    #[inline(always)]
    pub(crate) fn set_running(&self, running: bool) {
        let flags = self.flags.get();
        self.flags.set(if running {
            flags.with(NodeFlags::RUNNING)
        } else {
            flags.without(NodeFlags::RUNNING)
        });
    }

    #[inline(always)]
    pub(crate) fn bump_version(&self) {
        self.version.set(self.version.get().wrapping_add(1));
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
    pub(crate) node_aux: SparseSecondaryMap<RefCell<NodeAux>, 32>,
    pub(crate) reactive: SparseSecondaryMap<ReactiveNode, 64>,
    /// 非响应式节点（stored value / callback / node-ref）的载荷。
    ///
    /// 这里曾经是一个五变体的枚举 `ExtraData { Callback, NodeRef, StoredValue,
    /// Closure, Op }`（审计报告 §3.1 / §3.2 / §2.4），阶段二收敛成一个
    /// `Payload { value: AnyValue, borrowed: bool }`。阶段三连那个 `borrowed`
    /// 也去掉了：`None` 就是“正被某个用户闭包借出”，与 signal 的表示一致。
    pub(crate) extras: SparseSecondaryMap<RefCell<Option<AnyValue>>, 32>,

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

    /// 借一个响应式节点。
    ///
    /// 返回的引用只带共享权限 —— 改状态用 `Cell`、改载荷用 `RefCell`。
    /// 它唯一的规则是不得跨越**本节点自己**的销毁（见
    /// [`crate::internal::arena`] 的模块文档）。
    #[inline(always)]
    pub(crate) fn node(&self, id: NodeId) -> Option<&ReactiveNode> {
        self.reactive.get(id)
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
            self.node_aux.insert(id, RefCell::new(NodeAux::default()));
        }
        let aux = self.node_aux.get(id)?;
        Some(f(&mut aux.borrow_mut()))
    }

    #[inline(always)]
    pub(crate) fn get_state(&self, id: NodeId) -> NodeState {
        self.node(id).map_or(NodeState::Clean, |n| n.state.get())
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
    #[inline(always)]
    pub(crate) fn set_state(&self, id: NodeId, state: NodeState) {
        if let Some(node) = self.node(id) {
            node.state.set(state);
        }
    }

    #[inline(always)]
    pub(crate) fn is_running(&self, id: NodeId) -> bool {
        self.node(id).is_some_and(ReactiveNode::is_running)
    }

    pub(crate) fn describe(&self, id: NodeId) -> String {
        // release 构建下既没有调试标签也没有定义位置，只剩下编号。
        #[allow(unused_mut)]
        let mut out = format!("节点 #{}", id.slot());
        #[cfg(debug_assertions)]
        {
            if let Some(label) = self
                .node_aux
                .get(id)
                .and_then(|aux| aux.borrow().debug_label.clone())
            {
                out.push_str(&format!(" “{label}”"));
            }
            if let Some(at) = self.graph.get(id).and_then(|n| n.defined_at) {
                out.push_str(&format!("（定义于 {}:{}）", at.file(), at.line()));
            }
        }
        out
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{internal::value::EffectThunk, runtime::Runtime};

    #[test]
    fn set_state_never_inserts_a_ghost_node() {
        let storage = Storage::new();
        let id = storage.graph.insert(Node::new());

        // 这个节点还没有 `reactive` 条目。
        storage.set_state(id, NodeState::Dirty);

        assert!(
            storage.node(id).is_none(),
            "不得为不存在的节点插入永远不会被回收的幽灵条目（AUDIT P14）"
        );
        assert_eq!(storage.get_state(id), NodeState::Clean);
    }

    /// 订阅者表里残留一个已销毁的 id 时，传播不得为它造出条目。
    #[test]
    fn propagating_to_a_disposed_subscriber_leaves_nothing_behind() {
        let rt = Runtime::new();
        let s = rt.create_signal(AnyValue::new(1i32));

        let dead = rt.create_effect(EffectThunk::new(|| {}));
        rt.dispose(dead);
        assert!(rt.storage.node(dead).is_none());

        // 手工模拟“订阅者表里残留了一个已销毁的 id”。
        rt.storage
            .node(s)
            .expect("signal 还活着")
            .signal
            .borrow_mut()
            .subscribers
            .push(dead);

        rt.notify_update(s);

        assert!(
            rt.storage.node(dead).is_none(),
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

    /// 元数据是 `Cell`：持有一个节点的引用时改**另一个**节点的状态是合法的。
    /// 这正是订阅表 / 依赖表得以原地遍历的前提（审计报告 §3.3）。
    #[test]
    fn metadata_is_writable_through_a_shared_borrow() {
        let rt = Runtime::new();
        let a = rt.create_signal(AnyValue::new(1i32));
        let b = rt.create_signal(AnyValue::new(2i32));

        let node_a = rt.storage.node(a).expect("a 活着");
        rt.storage.set_state(b, NodeState::Dirty);
        node_a.state.set(NodeState::Check);

        assert_eq!(rt.storage.get_state(a), NodeState::Check);
        assert_eq!(rt.storage.get_state(b), NodeState::Dirty);
    }
}
