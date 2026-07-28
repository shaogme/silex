//! 响应式节点的 SoA (Structure of Arrays) 分离存储表示。

use crate::{
    DependencyList,
    internal::{
        arena::{Arena, RawId, SparseSecondaryMap},
        value::{AnyValue, Computation, OnceThunk},
    },
    runtime::graph::NodeState,
};
use std::{mem, vec::IntoIter};

/// 一个节点的种类与运行状态标志，压缩存放在单字节中。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct NodeFlags(u8);

impl NodeFlags {
    /// 包含可读值（Signal / Memo / Derived）。
    pub(crate) const VALUE: Self = Self(1 << 0);
    /// 包含计算闭包（Effect / Memo / Derived）。
    pub(crate) const COMPUTATION: Self = Self(1 << 1);
    /// 节点计算正在执行中（防止递归重入破坏依赖图关系）。
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

/// 节点的热元数据表条目。
pub(crate) struct NodeMeta {
    pub(crate) state: NodeState,
    pub(crate) flags: NodeFlags,
    pub(crate) version: u32,
    pub(crate) effect_version: u32,
    pub(crate) last_tracked_by: Option<(RawId, u32)>,
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

/// 节点的图拓扑边（订阅者列表与依赖关系列表）。
#[derive(Default)]
pub(crate) struct NodeLinks {
    pub(crate) subscribers: Vec<RawId>,
    pub(crate) dependencies: DependencyList,
}

/// 从存储集中提取出的完整节点载荷组合。
#[expect(dead_code, reason = "用于在墓园中持有并延后析构完整节点载荷")]
pub(crate) struct NodeParts {
    pub(crate) meta: NodeMeta,
    pub(crate) links: NodeLinks,
    pub(crate) value: Option<AnyValue>,
    pub(crate) computation: Option<Computation>,
}

/// 最多为多少个已销毁节点保留调试标签上限（仅 Debug 构建）。
#[cfg(debug_assertions)]
pub(crate) const MAX_DEAD_NODE_LABELS: usize = 1024;

/// 运行时集中存储所有响应式节点数据的 SoA 结构体。
pub(crate) struct Storage {
    pub(crate) graph: Arena<Node>,
    pub(crate) node_aux: SparseSecondaryMap<NodeAux, 32>,
    pub(crate) meta: SparseSecondaryMap<NodeMeta, 64>,
    pub(crate) links: SparseSecondaryMap<NodeLinks, 64>,
    pub(crate) values: SparseSecondaryMap<Option<AnyValue>, 64>,
    pub(crate) computations: SparseSecondaryMap<Option<Computation>, 64>,
    /// 非响应式节点（stored value / callback / node-ref）的载荷存储。
    pub(crate) extras: SparseSecondaryMap<Option<AnyValue>, 32>,

    /// 延后析构墓园：存放已被从图解构、但等待在 Runtime 借用之外析构的用户残骸。
    graveyard: Vec<Debris>,

    #[cfg(debug_assertions)]
    pub(crate) dead_node_labels: SparseSecondaryMap<String>,
    #[cfg(debug_assertions)]
    dead_label_count: usize,
}

/// 已经被从响应式图中摘除、但等待在 Runtime 借用之外执行用户 `Drop` 的残骸。
///
/// 包含用户自定义类型的值、计算闭包及 Cleanup。将其放入墓园可保障用户 `Drop` 代码在
/// Runtime 借用之外被安全调用，防止 `Drop` 重入触发借用冲突。
#[expect(
    dead_code,
    reason = "载荷只为了『在墓园里、而不是在借用之内被析构』而存在，没有读取者"
)]
pub(crate) enum Debris {
    /// 包含元数据、值、闭包与边表在内的完整响应式节点。
    Node(NodeParts),
    /// 尚未执行的 Cleanup 及 Scope 冷数据。
    Aux(NodeAux),
    /// 非响应式载荷或被覆盖更新掉的旧值。
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

    /// 记录已销毁节点的调试标签（Debug 模式下受到上限控制）。
    #[cfg(debug_assertions)]
    pub(crate) fn remember_dead_label(&mut self, id: RawId, label: String) {
        let count = self.dead_label_count;
        if count >= MAX_DEAD_NODE_LABELS {
            return;
        }
        self.dead_label_count = count + 1;
        self.dead_node_labels.insert(id, label);
    }

    /// 将残骸入列墓园，以便稍后在 Runtime 借用释放后排空析构。
    #[inline]
    pub(crate) fn bury(&mut self, debris: Debris) {
        self.graveyard.push(debris);
    }

    /// 提取一件待析构的墓园残骸。
    #[inline]
    pub(crate) fn take_debris(&mut self) -> Option<Debris> {
        self.graveyard.pop()
    }

    pub(crate) fn insert_reactive(
        &mut self,
        id: RawId,
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

    pub(crate) fn remove_reactive(&mut self, id: RawId) -> Option<NodeParts> {
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
    pub(crate) fn meta(&self, id: RawId) -> Option<&NodeMeta> {
        self.meta.get(id)
    }

    #[inline(always)]
    pub(crate) fn meta_mut(&mut self, id: RawId) -> Option<&mut NodeMeta> {
        self.meta.get_mut(id)
    }

    #[inline(always)]
    pub(crate) fn value(&self, id: RawId) -> Option<&AnyValue> {
        self.values.get(id)?.as_ref()
    }

    #[inline(always)]
    pub(crate) fn value_mut(&mut self, id: RawId) -> Option<&mut Option<AnyValue>> {
        self.values.get_mut(id)
    }

    #[inline(always)]
    pub(crate) fn computation_mut(&mut self, id: RawId) -> Option<&mut Option<Computation>> {
        self.computations.get_mut(id)
    }

    /// 在闭包作用域内访问节点的辅助冷数据，必要时自动创建默认项。
    pub(crate) fn with_aux_mut<R>(
        &mut self,
        id: RawId,
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
    pub(crate) fn get_state(&self, id: RawId) -> NodeState {
        self.meta(id).map_or(NodeState::Clean, |node| node.state)
    }

    /// 更新已知节点的计算求值状态（已销毁节点将安全忽略，防止插入孤立内存）。
    #[inline(always)]
    pub(crate) fn set_state(&mut self, id: RawId, state: NodeState) {
        if let Some(node) = self.meta_mut(id) {
            node.state = state;
        }
    }

    #[inline(always)]
    pub(crate) fn is_running(&self, id: RawId) -> bool {
        self.meta(id).is_some_and(NodeMeta::is_running)
    }

    pub(crate) fn describe(&self, id: RawId) -> String {
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
    pub(crate) children: Vec<RawId>,
    pub(crate) cleanups: CleanupList,
    #[cfg(debug_assertions)]
    pub(crate) debug_label: Option<String>,
}

/// 响应式节点通用结构体 (Metadata)。
/// 仅保留最核心的“热数据”以减小体积。
pub(crate) struct Node {
    pub(crate) parent: Option<RawId>,
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
            drive::dispose_raw(dead);

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
                drive::dispose_raw(id);
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
