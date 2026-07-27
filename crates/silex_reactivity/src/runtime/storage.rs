//! 节点的存储表示。
//!
//! # 阶段三：SoA 与独占访问
//!
//! 响应式节点按元数据、图边、值和计算闭包拆成独立表。运行时入口交出
//! `&mut Runtime`，因此普通内部路径不再依赖节点内的内部可变性：
//!
//! | 分类 | 存法 | 为什么 |
//! |---|---|---|
//! | **元数据**（状态、版本、标志） | 独立表 | 传播只借用元数据表，能同时遍历另一张 links 表 |
//! | **载荷**（值、闭包、依赖表、订阅表） | 独立表 | 用户代码执行前统一移出载荷，普通内部路径只使用独占借用 |
//!
//! 两个“借出中”的布尔标志也随之收敛：值与闭包都用 `Option` 表示借出，
//! 借出期间节点里是 `None` 而不再是一个现造的 `AnyValue::placeholder()`。

use crate::{
    DependencyList,
    internal::{
        arena::{Arena, Index as NodeId, SparseSecondaryMap},
        value::{AnyValue, Computation, OnceThunk},
    },
    runtime::graph::NodeState,
};
use std::{mem, vec::IntoIter};

/// 一个节点的种类与运行状态，打包进一个字节。
///
/// 这些位从前分散成不同载荷表的存在性，判定种类因此要扫描载荷；现在状态与
/// 种类位集中在独立的热元数据表中。
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

/// 节点的热元数据。它与所有可变载荷分离，传播可以只借用这张表。
pub(crate) struct NodeMeta {
    pub(crate) state: NodeState,
    pub(crate) flags: NodeFlags,
    pub(crate) version: u32,
    pub(crate) effect_version: u32,
    pub(crate) last_tracked_by: Option<(NodeId, u32)>,
}

impl NodeMeta {
    pub(crate) fn new(state: NodeState, flags: NodeFlags) -> Self {
        Self {
            state,
            flags,
            version: 0,
            effect_version: 0,
            last_tracked_by: None,
        }
    }

    #[inline(always)]
    pub(crate) fn has_value(&self) -> bool {
        self.flags.has(NodeFlags::VALUE)
    }

    #[inline(always)]
    pub(crate) fn is_computation(&self) -> bool {
        self.flags.has(NodeFlags::COMPUTATION)
    }

    #[inline(always)]
    pub(crate) fn is_effect(&self) -> bool {
        self.flags.has(NodeFlags::COMPUTATION) && !self.flags.has(NodeFlags::VALUE)
    }

    #[inline(always)]
    pub(crate) fn is_running(&self) -> bool {
        self.flags.has(NodeFlags::RUNNING)
    }

    #[inline(always)]
    pub(crate) fn set_running(&mut self, running: bool) {
        self.flags = if running {
            self.flags.with(NodeFlags::RUNNING)
        } else {
            self.flags.without(NodeFlags::RUNNING)
        };
    }

    #[inline(always)]
    pub(crate) fn bump_version(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

/// 节点的图边。订阅者和依赖关系与值、闭包分开存放。
#[derive(Default)]
pub(crate) struct NodeLinks {
    pub(crate) subscribers: Vec<NodeId>,
    pub(crate) dependencies: DependencyList,
}

/// 从响应式存储中移出的完整节点载荷。
#[expect(dead_code, reason = "墓园只负责持有并析构完整节点载荷")]
pub(crate) struct NodeParts {
    pub(crate) meta: NodeMeta,
    pub(crate) links: NodeLinks,
    pub(crate) value: Option<AnyValue>,
    pub(crate) computation: Option<Computation>,
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
    pub(crate) meta: SparseSecondaryMap<NodeMeta, 64>,
    pub(crate) links: SparseSecondaryMap<NodeLinks, 64>,
    pub(crate) values: SparseSecondaryMap<Option<AnyValue>, 64>,
    pub(crate) computations: SparseSecondaryMap<Option<Computation>, 64>,
    /// 非响应式节点（stored value / callback / node-ref）的载荷。
    ///
    /// 这里曾经是一个五变体的枚举 `ExtraData { Callback, NodeRef, StoredValue,
    /// Closure, Op }`（审计报告 §3.1 / §3.2 / §2.4），阶段二收敛成一个
    /// `Payload { value: AnyValue, borrowed: bool }`。阶段三连那个 `borrowed`
    /// 也去掉了：`None` 就是“正被某个用户闭包借出”，与 signal 的表示一致。
    pub(crate) extras: SparseSecondaryMap<Option<AnyValue>, 32>,

    /// 已经离开图、等着在**借用之外**析构的残骸。见 [`Debris`]。
    graveyard: Vec<Debris>,

    #[cfg(debug_assertions)]
    pub(crate) dead_node_labels: SparseSecondaryMap<String>,
    /// 已记下的墓碑标签数量的上界（同一个槽位被覆盖时会多算，只用于封顶）。
    #[cfg(debug_assertions)]
    dead_label_count: usize,
}

/// 一件已经从图里摘下来、但**还没有析构**的东西。
///
/// 这三种残骸里装的都是用户的数据：signal / memo 的值、effect 与 memo 的
/// 计算闭包、尚未执行的 cleanup。析构它们就是执行用户的 `Drop`，而用户的
/// `Drop` 完全可以回头访问响应式图 —— 销毁别的节点、写一个 signal、
/// 甚至再建一个 effect。
///
/// 就地析构意味着那段用户代码运行在运行时的借用之内。今天这只在一个偏门角落
/// 出事（`commit_update` 覆盖旧值时握着节点的 `borrow_mut`，一个会读自己所在
/// memo 的 `T::drop` 会撞上 `already borrowed`）；方案 B 把访问入口收成
/// `&mut Runtime` 之后，**每一处**就地析构都会变成一次借用冲突。
///
/// 所以规则改成：让值离开图的地方一律把它推进墓园，由驱动循环在释放借用之后
/// 调 [`Storage::drain_graveyard`]。排空点选在与从前析构时机**相同**的位置上，
/// 因此用户可观察的顺序不变。
#[expect(
    dead_code,
    reason = "载荷只为了『在墓园里、而不是在借用之内被析构』而存在，没有读取者"
)]
pub(crate) enum Debris {
    /// 一个响应式节点的全部载荷：元数据、值、计算闭包与两张边表。
    Node(NodeParts),
    /// 冷数据：尚未执行的 cleanup（子节点列表里只有 id，不带用户数据）。
    Aux(NodeAux),
    /// 非响应式载荷（stored value / callback / node-ref），以及被覆盖掉的旧值。
    Payload(AnyValue),
}

impl Storage {
    pub(crate) fn new() -> Self {
        Self {
            graph: Arena::new(),
            node_aux: SparseSecondaryMap::new(),
            meta: SparseSecondaryMap::new(),
            links: SparseSecondaryMap::new(),
            values: SparseSecondaryMap::new(),
            computations: SparseSecondaryMap::new(),
            extras: SparseSecondaryMap::new(),
            graveyard: Vec::new(),
            #[cfg(debug_assertions)]
            dead_node_labels: SparseSecondaryMap::new(),
            #[cfg(debug_assertions)]
            dead_label_count: 0,
        }
    }

    /// 为一个即将被销毁的节点留一个墓碑标签，数量封顶（见 [`MAX_DEAD_NODE_LABELS`]）。
    #[cfg(debug_assertions)]
    pub(crate) fn remember_dead_label(&mut self, id: NodeId, label: String) {
        let count = self.dead_label_count;
        if count >= MAX_DEAD_NODE_LABELS {
            return;
        }
        self.dead_label_count = count + 1;
        self.dead_node_labels.insert(id, label);
    }

    /// 把一件残骸交给墓园，等驱动循环在借用之外析构它。
    #[inline]
    pub(crate) fn bury(&mut self, debris: Debris) {
        self.graveyard.push(debris);
    }

    /// 取一件残骸出来。
    ///
    /// 析构**不在这里发生** —— 那是用户的 `Drop`，只能跑在借用之外。
    /// 排空循环见 [`drain_graveyard`](crate::runtime::drive::drain_graveyard)。
    #[inline]
    pub(crate) fn take_debris(&mut self) -> Option<Debris> {
        self.graveyard.pop()
    }

    pub(crate) fn insert_reactive(
        &mut self,
        id: NodeId,
        meta: NodeMeta,
        links: NodeLinks,
        value: Option<AnyValue>,
        computation: Option<Computation>,
    ) {
        let meta_inserted = self.meta.insert(id, meta);
        let links_inserted = self.links.insert(id, links);
        let value_inserted = self.values.insert(id, value);
        let computation_inserted = self.computations.insert(id, computation);
        debug_assert!(meta_inserted);
        debug_assert!(links_inserted);
        debug_assert!(value_inserted);
        debug_assert!(computation_inserted);
    }

    pub(crate) fn remove_reactive(&mut self, id: NodeId) -> Option<NodeParts> {
        let meta = self.meta.remove(id)?;
        let links = self.links.remove(id).unwrap_or_default();
        let value = self.values.remove(id).unwrap_or(None);
        let computation = self.computations.remove(id).unwrap_or(None);
        Some(NodeParts {
            meta,
            links,
            value,
            computation,
        })
    }

    #[inline(always)]
    pub(crate) fn meta(&self, id: NodeId) -> Option<&NodeMeta> {
        self.meta.get(id)
    }

    #[inline(always)]
    pub(crate) fn meta_mut(&mut self, id: NodeId) -> Option<&mut NodeMeta> {
        self.meta.get_mut(id)
    }

    #[inline(always)]
    pub(crate) fn value(&self, id: NodeId) -> Option<&AnyValue> {
        self.values.get(id)?.as_ref()
    }

    #[inline(always)]
    pub(crate) fn value_mut(&mut self, id: NodeId) -> Option<&mut Option<AnyValue>> {
        self.values.get_mut(id)
    }

    #[inline(always)]
    pub(crate) fn computation_mut(&mut self, id: NodeId) -> Option<&mut Option<Computation>> {
        self.computations.get_mut(id)
    }

    /// 在闭包作用域内可变地访问一个节点的冷数据，必要时先建出来。
    ///
    /// 节点不在 `graph` 里（已销毁 / 伪造的句柄）时返回 `None` 且不建任何条目。
    pub(crate) fn with_aux_mut<R>(
        &mut self,
        id: NodeId,
        f: impl FnOnce(&mut NodeAux) -> R,
    ) -> Option<R> {
        if !self.node_aux.contains_key(id) {
            self.graph.get(id)?;
            self.node_aux.insert(id, NodeAux::default());
        }
        let aux = self.node_aux.get_mut(id)?;
        Some(f(aux))
    }

    #[inline(always)]
    pub(crate) fn get_state(&self, id: NodeId) -> NodeState {
        self.meta(id).map_or(NodeState::Clean, |node| node.state)
    }

    /// 只更新已存在的节点。
    ///
    /// 之前这里会为不存在的节点**插入**一个空的响应式条目：订阅者表里只要
    /// 残留了一个已销毁的 id（`propagate` 遍历时就会遇到），就会为它造出一个
    /// 既不在 `graph` 里、也不会被任何 dispose 路径清理的幽灵条目 —— 长跑的
    /// 应用会一直泄漏下去（AUDIT P14）。
    ///
    /// 忽略掉是安全的：`get_state` 对不存在的节点返回 `Clean`，
    /// 传播与求值都会把它当成“无需处理”。
    #[inline(always)]
    pub(crate) fn set_state(&mut self, id: NodeId, state: NodeState) {
        if let Some(node) = self.meta_mut(id) {
            node.state = state;
        }
    }

    #[inline(always)]
    pub(crate) fn is_running(&self, id: NodeId) -> bool {
        self.meta(id).is_some_and(NodeMeta::is_running)
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
                .and_then(|aux| aux.debug_label.clone())
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
    use crate::{
        internal::value::EffectThunk,
        runtime::{drive, with_rt_or_init},
    };

    /// 在一条**新线程**上跑用例：运行时是线程本地的，而这些用例要在一张
    /// 干净的图上断言。
    fn on_a_fresh_runtime(f: impl FnOnce() + Send + 'static) {
        std::thread::spawn(f).join().expect("用例线程不应 panic");
    }

    #[test]
    fn set_state_never_inserts_a_ghost_node() {
        let mut storage = Storage::new();
        let id = storage.graph.insert(Node::new());

        // 这个节点还没有 `reactive` 条目。
        storage.set_state(id, NodeState::Dirty);

        assert!(
            storage.meta(id).is_none(),
            "不得为不存在的节点插入永远不会被回收的幽灵条目（AUDIT P14）"
        );
        assert_eq!(storage.get_state(id), NodeState::Clean);
    }

    /// 订阅者表里残留一个已销毁的 id 时，传播不得为它造出条目。
    #[test]
    fn propagating_to_a_disposed_subscriber_leaves_nothing_behind() {
        on_a_fresh_runtime(|| {
            let s = drive::create_signal(AnyValue::new(1i32)).expect("运行时可用");
            let dead = drive::create_effect(EffectThunk::new(|| {})).expect("运行时可用");
            drive::dispose(dead);

            with_rt_or_init(|rt| {
                assert!(rt.storage.meta(dead).is_none());
                // 手工模拟“订阅者表里残留了一个已销毁的 id”。
                rt.storage
                    .links
                    .get_mut(s)
                    .expect("signal 还活着")
                    .subscribers
                    .push(dead);
            })
            .expect("运行时可用");

            drive::notify_update(s);

            assert!(
                with_rt_or_init(|rt| rt.storage.meta(dead).is_none()).expect("运行时可用"),
                "传播到已销毁的订阅者时不得复活它（AUDIT P14）"
            );
        });
    }

    /// 墓碑标签只在 debug 构建下存在。
    #[cfg(debug_assertions)]
    #[test]
    fn dead_node_labels_are_capped() {
        on_a_fresh_runtime(|| {
            for i in 0..(MAX_DEAD_NODE_LABELS + 16) {
                let id = drive::create_signal(AnyValue::new(i)).expect("运行时可用");
                with_rt_or_init(|rt| {
                    rt.storage
                        .with_aux_mut(id, |aux| aux.debug_label = Some(format!("node-{i}")));
                })
                .expect("运行时可用");
                drive::dispose(id);
            }
            assert_eq!(
                with_rt_or_init(|rt| rt.storage.dead_label_count).expect("运行时可用"),
                MAX_DEAD_NODE_LABELS
            );
        });
    }

    /// SoA 允许在借用 links 表时独占修改另一张 meta 表。
    #[test]
    fn metadata_is_writable_through_a_shared_borrow() {
        on_a_fresh_runtime(|| {
            let a = drive::create_signal(AnyValue::new(1i32)).expect("运行时可用");
            let b = drive::create_signal(AnyValue::new(2i32)).expect("运行时可用");

            with_rt_or_init(|rt| {
                rt.storage.set_state(b, NodeState::Dirty);
                rt.storage.meta_mut(a).expect("a 活着").state = NodeState::Check;

                assert_eq!(rt.storage.get_state(a), NodeState::Check);
                assert_eq!(rt.storage.get_state(b), NodeState::Dirty);
            })
            .expect("运行时可用");
        });
    }
}
