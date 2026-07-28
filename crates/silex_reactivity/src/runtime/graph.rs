//! 响应式图算法：通知传播（BFS）与增量求值（DFS）。

use crate::{
    internal::arena::RawId,
    runtime::{
        Runtime,
        scheduler::Scheduler,
        storage::{NodeLinks, NodeMeta, Storage},
    },
};
use std::collections::VecDeque;

/// 节点在计算求值过程中的状态。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NodeState {
    /// 节点已是最新的，无需重新计算。
    Clean,
    /// 依赖的节点已被标记为变更，需检查上游版本号确定是否需要重算。
    Check,
    /// 节点已被直接标脏，必须重新计算。
    Dirty,
}

/// [`Runtime::eval_step`] 步进求值返回的结果指示。
///
/// 求值 DFS 仅负责状态推导与遍历，不直接执行用户闭包。
/// 遇到需计算的节点时，交还节点句柄由外部驱动层在释放 Runtime 借用后执行。
#[derive(Clone, Copy)]
pub(crate) enum Step {
    /// 目标节点的 DFS 求值遍历已全部完成。
    Done,
    /// 需在 Runtime 借用之外执行指定节点的计算闭包。
    Run(RawId),
}

/// 增量求值 DFS 栈帧。
#[derive(Clone, Copy)]
pub(crate) struct EvalFrame {
    node: RawId,
    /// 当前节点依赖列表中下一个待扫描的下标索引（避免重复扫描全量依赖表）。
    cursor: usize,
}

impl EvalFrame {
    #[inline]
    pub(crate) fn new(node: RawId) -> Self {
        Self { node, cursor: 0 }
    }
}

impl Runtime {
    // --- 拓扑遍历辅助 ---

    /// 就地遍历指定节点的直接订阅者列表 (`subscribers`)。
    #[inline]
    fn for_each_subscriber(
        links: &crate::internal::arena::SparseSecondaryMap<NodeLinks, 64>,
        id: RawId,
        mut f: impl FnMut(RawId),
    ) {
        let Some(node_links) = links.get(id) else {
            return;
        };
        for &sub_id in node_links.subscribers.as_slice() {
            f(sub_id);
        }
    }

    /// 从 `from` 下标开始寻找 `id` 的依赖表中第一个非 `Clean` 且不在运行中的依赖节点。
    fn next_unclean_dependency(&self, id: RawId, from: usize) -> Option<(RawId, usize)> {
        let deps = self.storage.links.get(id)?.dependencies.as_slice();
        let start = from.min(deps.len());
        for (offset, &(dep_id, _, _)) in deps.iter().enumerate().skip(start) {
            if self.storage.get_state(dep_id) != NodeState::Clean
                && !self.storage.is_running(dep_id)
            {
                return Some((dep_id, offset));
            }
        }
        None
    }

    /// 检查节点的上游依赖版本号自上次计算后是否发生变动。
    fn dependencies_changed(&self, id: RawId) -> bool {
        let Some(dependencies) = self.storage.links.get(id) else {
            return false;
        };
        dependencies
            .dependencies
            .as_slice()
            .iter()
            .any(|&(dep_id, expected, _)| match self.storage.meta(dep_id) {
                Some(dep) if dep.has_value() => dep.version != expected,
                _ => true,
            })
    }

    // --- 第一阶段：BFS 传播 (Propagation) ---

    /// 从 `start_node` 开始执行 BFS 传播：
    /// - 将直接订阅者标记为 `Dirty`，间接订阅者标记为 `Check`。
    /// - 将途经的副作用节点 (Effect) 放入调度器队列，终止对 Effect 下游的推演。
    pub(crate) fn propagate(&mut self, start_node: RawId, queue: &mut VecDeque<RawId>) {
        queue.clear();

        let Runtime {
            storage, scheduler, ..
        } = self;
        let Storage { meta, links, .. } = storage;

        Self::for_each_subscriber(links, start_node, |sub_id| {
            if meta
                .get(sub_id)
                .is_some_and(|node| node.state != NodeState::Dirty)
            {
                if let Some(node) = meta.get_mut(sub_id) {
                    node.state = NodeState::Dirty;
                }
                Self::schedule_or_walk(meta, scheduler, sub_id, queue);
            }
        });

        while let Some(current_id) = queue.pop_front() {
            Self::for_each_subscriber(links, current_id, |sub_id| {
                if meta
                    .get(sub_id)
                    .is_some_and(|node| node.state == NodeState::Clean)
                {
                    if let Some(node) = meta.get_mut(sub_id) {
                        node.state = NodeState::Check;
                    }
                    Self::schedule_or_walk(meta, scheduler, sub_id, queue);
                }
            });
        }
    }

    #[inline]
    fn schedule_or_walk(
        meta: &crate::internal::arena::SparseSecondaryMap<NodeMeta, 64>,
        scheduler: &mut Scheduler,
        id: RawId,
        queue: &mut VecDeque<RawId>,
    ) {
        if meta.get(id).is_some_and(NodeMeta::is_effect) {
            scheduler.queue_effect(id);
        } else {
            queue.push_back(id);
        }
    }

    // --- 第二阶段：DFS 增量求值 (Evaluation) ---

    /// 推进单步求值 DFS。
    ///
    /// 沿着依赖关系向上搜索未计算上游：
    /// - 若发现未 Clean 上游，压入栈中并继续推导。
    /// - 若发现循环依赖，触发 panic。
    /// - 若上游均已 Clean，检查版本号：变动则返回 `Step::Run(id)` 交由驱动层计算，未变动则设为 `Clean` 并弹栈。
    ///
    /// # Panics
    ///
    /// 当检测到响应式依赖图中存在环路 (Cycle) 时抛出 panic。
    pub(crate) fn eval_step(&mut self, stack: &mut Vec<EvalFrame>) -> Step {
        while let Some(&EvalFrame {
            node: current,
            cursor,
        }) = stack.last()
        {
            let state = self.storage.get_state(current);

            if state == NodeState::Clean || self.storage.is_running(current) {
                stack.pop();
                continue;
            }

            let found = match self.next_unclean_dependency(current, cursor) {
                hit @ Some(_) => hit,
                None if cursor > 0 => self.next_unclean_dependency(current, 0),
                None => None,
            };

            if let Some((dep_id, at)) = found {
                if stack.iter().any(|frame| frame.node == dep_id) {
                    panic!(
                        "silex_reactivity: 检测到依赖环，无法求值。环上的节点：\n    {}",
                        self.describe_cycle(stack, dep_id)
                    );
                }
                if let Some(frame) = stack.last_mut() {
                    frame.cursor = at;
                }
                stack.push(EvalFrame::new(dep_id));
                continue;
            }

            if state == NodeState::Check && !self.dependencies_changed(current) {
                self.storage.set_state(current, NodeState::Clean);
                stack.pop();
                continue;
            }

            return Step::Run(current);
        }

        Step::Done
    }

    /// 格式化输出依赖环路的诊断信息。
    fn describe_cycle(&self, stack: &[EvalFrame], repeated: RawId) -> String {
        let start = stack
            .iter()
            .position(|frame| frame.node == repeated)
            .unwrap_or(stack.len());
        let mut chain: Vec<String> = stack[start..]
            .iter()
            .map(|frame| self.storage.describe(frame.node))
            .collect();
        chain.push(self.storage.describe(repeated));
        chain.join("\n    -> 依赖 ")
    }
}

#[cfg(test)]
mod tests {
    //! 图算法的单元测试 —— 现在直接搭在真实的 `Runtime` 上（AUDIT P18 的用例，
    //! 阶段三随抽象层一起迁移，见模块文档）。

    use super::*;
    use crate::{
        internal::value::{AnyValue, Computation, EffectThunk, MemoThunk},
        runtime::{
            drive,
            storage::{NodeFlags, NodeLinks, NodeMeta},
            with_rt_or_init,
        },
    };
    use std::{cell::RefCell, rc::Rc};

    /// 在一条**新线程**上跑用例。
    ///
    /// 运行时是线程本地的，而这些用例要在一张干净的图上断言绝对的执行顺序。
    /// 从前它们各自 `Runtime::new()` 一个独立实例；驱动层改走 `with_rt` 之后
    /// 不可能再这么做（驱动够不到一个游离的 `Runtime`），换线程是等价的隔离。
    fn on_a_fresh_runtime(f: impl FnOnce() + Send + 'static) {
        std::thread::spawn(f).join().expect("用例线程不应 panic");
    }

    /// 手工搭图的脚手架：节点的种类、状态、依赖边都由用例直接指定，
    /// 计算闭包只记录“我被跑过了”。
    struct Graph {
        ran: Rc<RefCell<Vec<RawId>>>,
    }

    impl Graph {
        fn new() -> Self {
            Self {
                ran: Rc::new(RefCell::new(Vec::new())),
            }
        }

        /// 只有值、没有计算：一个普通 signal。
        fn signal(&self) -> RawId {
            drive::create_signal(AnyValue::new(0i32)).expect("运行时可用")
        }

        /// 计算闭包只记录“我被跑过了”。
        ///
        /// 带值的节点（memo）必须装一个 `MemoThunk` —— 它的返回值会被
        /// `recompute_memo` 提交进节点，因此这里恒返回同一个常量：
        /// 首算时没有旧值，一律算“变了”；之后每次都与旧值相等，不再惊动下游。
        /// 用例只看 `ran`，这个选择让它们不受提交语义干扰。
        fn computation(&self, flags: NodeFlags) -> RawId {
            let ran = self.ran.clone();
            with_rt_or_init(|rt| {
                let id = rt.register_node_at(std::panic::Location::caller());
                let thunk = if flags.has(NodeFlags::VALUE) {
                    Computation::Memo(MemoThunk::new::<i32, _>(move |_| {
                        ran.borrow_mut().push(id);
                        0
                    }))
                } else {
                    Computation::Effect(EffectThunk::new(move || ran.borrow_mut().push(id)))
                };
                rt.storage.insert_reactive(
                    id,
                    NodeMeta::new(NodeState::Clean, flags),
                    NodeLinks::default(),
                    flags.has(NodeFlags::VALUE).then(|| AnyValue::new(0i32)),
                    Some(thunk),
                );
                id
            })
            .expect("运行时可用")
        }

        /// 既有值又有计算：memo / derived。
        fn memo(&self) -> RawId {
            self.computation(NodeFlags::VALUE.with(NodeFlags::COMPUTATION))
        }

        /// 只有计算：effect。
        fn effect(&self) -> RawId {
            self.computation(NodeFlags::COMPUTATION)
        }

        /// `from` 的变化会传播到 `to`（即 `to` 依赖 `from`）。
        fn edge(&self, from: RawId, to: RawId) {
            with_rt_or_init(|rt| {
                let version = rt.storage.meta(from).map_or(0, |node| node.version);
                let subscribers = &mut rt
                    .storage
                    .links
                    .get_mut(from)
                    .expect("from 必须存在")
                    .subscribers;
                let subscriber_index = subscribers.len();
                subscribers.push(to);
                rt.storage
                    .links
                    .get_mut(to)
                    .expect("to 必须存在")
                    .dependencies
                    .push((from, version, subscriber_index));
            })
            .expect("运行时可用");
        }

        fn state(&self, id: RawId) -> NodeState {
            with_rt_or_init(|rt| rt.storage.get_state(id)).expect("运行时可用")
        }

        fn set_state(&self, id: RawId, state: NodeState) {
            with_rt_or_init(|rt| rt.storage.set_state(id, state)).expect("运行时可用");
        }

        fn set_running(&self, id: RawId, running: bool) {
            with_rt_or_init(|rt| {
                rt.storage
                    .meta_mut(id)
                    .expect("节点必须存在")
                    .set_running(running);
            })
            .expect("运行时可用");
        }

        fn alive(&self, id: RawId) -> bool {
            with_rt_or_init(|rt| rt.storage.meta(id).is_some()).expect("运行时可用")
        }

        fn ran(&self) -> Vec<RawId> {
            self.ran.borrow().clone()
        }

        fn propagate_from(&self, start: RawId) -> Vec<RawId> {
            with_rt_or_init(|rt| {
                let mut queue = VecDeque::new();
                rt.propagate(start, &mut queue);
                rt.scheduler.observer_queue.iter().copied().collect()
            })
            .expect("运行时可用")
        }

        fn evaluate_node(&self, target: RawId) {
            drive::drive_eval(target);
        }
    }

    // --- propagate ---

    #[test]
    fn propagate_marks_direct_subscribers_dirty_and_the_rest_check() {
        on_a_fresh_runtime(|| {
            // a -> b -> c
            let g = Graph::new();
            let (a, b, c) = (g.signal(), g.memo(), g.memo());
            g.edge(a, b);
            g.edge(b, c);

            g.propagate_from(a);

            assert_eq!(g.state(b), NodeState::Dirty, "直接订阅者是 Dirty");
            assert_eq!(g.state(c), NodeState::Check, "间接下游只是 Check");
        });
    }

    #[test]
    fn propagate_queues_effects_instead_of_walking_past_them() {
        on_a_fresh_runtime(|| {
            // a -> e(effect) -> c
            let g = Graph::new();
            let (a, e, c) = (g.signal(), g.effect(), g.memo());
            g.edge(a, e);
            g.edge(e, c);

            let queued = g.propagate_from(a);

            assert_eq!(queued, vec![e]);
            assert_eq!(
                g.state(c),
                NodeState::Clean,
                "effect 没有订阅者语义上的下游，传播到它为止"
            );
        });
    }

    #[test]
    fn propagate_does_not_downgrade_a_dirty_node_to_check() {
        on_a_fresh_runtime(|| {
            // a -> c, a -> b -> c：菱形的短边先把 c 标 Dirty，长边不能把它降成 Check
            let g = Graph::new();
            let (a, b, c) = (g.signal(), g.memo(), g.memo());
            g.edge(a, b);
            g.edge(a, c);
            g.edge(b, c);

            g.propagate_from(a);

            assert_eq!(g.state(c), NodeState::Dirty);
        });
    }

    #[test]
    fn propagate_ignores_nodes_that_no_longer_exist() {
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let a = g.signal();
            let dead = g.memo();
            g.edge(a, dead);
            drive::dispose_raw(dead);

            g.propagate_from(a);

            assert!(
                !g.alive(dead),
                "不得为已销毁的订阅者造出幽灵条目（AUDIT P14）"
            );
        });
    }

    // --- evaluate ---

    #[test]
    fn evaluating_a_clean_node_does_nothing() {
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let m = g.memo();

            g.evaluate_node(m);

            assert!(g.ran().is_empty());
        });
    }

    #[test]
    fn evaluate_runs_dependencies_before_dependents() {
        on_a_fresh_runtime(|| {
            // a -> b -> c，全部标脏后从 c 开始求值
            let g = Graph::new();
            let (a, b, c) = (g.memo(), g.memo(), g.memo());
            g.edge(a, b);
            g.edge(b, c);
            for id in [a, b, c] {
                g.set_state(id, NodeState::Dirty);
            }

            g.evaluate_node(c);

            assert_eq!(g.ran(), vec![a, b, c], "必须自上游向下游依次求值");
        });
    }

    #[test]
    fn a_check_node_whose_dependencies_did_not_change_is_not_recomputed() {
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let (a, b) = (g.signal(), g.memo());
            g.edge(a, b); // 依赖边记下的就是 a 当前的版本号
            g.set_state(b, NodeState::Check);

            g.evaluate_node(b);

            assert!(g.ran().is_empty(), "版本号没变就不该重算");
            assert_eq!(g.state(b), NodeState::Clean);
        });
    }

    #[test]
    fn a_check_node_whose_dependencies_changed_is_recomputed() {
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let (a, b) = (g.signal(), g.memo());
            g.edge(a, b);
            with_rt_or_init(|rt| rt.storage.meta_mut(a).expect("a 活着").bump_version())
                .expect("运行时可用");
            g.set_state(b, NodeState::Check);

            g.evaluate_node(b);

            assert_eq!(g.ran(), vec![b]);
        });
    }

    #[test]
    fn a_running_node_is_never_re_evaluated() {
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let m = g.memo();
            g.set_state(m, NodeState::Dirty);
            g.set_running(m, true);

            g.evaluate_node(m);

            assert!(
                g.ran().is_empty(),
                "正在运行的节点重跑由队列负责（AUDIT P1）"
            );
            assert_eq!(
                g.state(m),
                NodeState::Dirty,
                "它的失效标记必须保留，否则队列里的重跑条目会被当成已干净跳过"
            );
        });
    }

    #[test]
    fn a_running_dependency_does_not_block_its_dependent() {
        on_a_fresh_runtime(|| {
            // b 依赖 a，a 正在运行：不能停下来等它变干净（它不可能在本次 DFS 里变干净）
            let g = Graph::new();
            let (a, b) = (g.memo(), g.memo());
            g.edge(a, b);
            g.set_state(a, NodeState::Dirty);
            g.set_running(a, true);
            g.set_state(b, NodeState::Dirty);

            g.evaluate_node(b);

            assert_eq!(g.ran(), vec![b]);
        });
    }

    #[test]
    fn a_non_computation_node_is_forced_clean() {
        on_a_fresh_runtime(|| {
            // 一个被标脏的普通 signal：跑不了计算，必须被置 Clean，
            // 否则下游会反复把它压栈。
            let g = Graph::new();
            let (a, b) = (g.signal(), g.memo());
            g.edge(a, b);
            g.set_state(a, NodeState::Dirty);
            g.set_state(b, NodeState::Dirty);

            g.evaluate_node(b);

            assert_eq!(g.state(a), NodeState::Clean);
            assert_eq!(g.ran(), vec![b]);
        });
    }

    #[test]
    #[should_panic(expected = "用例线程不应 panic")]
    fn a_dependency_cycle_panics() {
        // a <-> b。环上的 panic 发生在用例线程里，`join()` 把它转成这里的失败；
        // 真正那句“依赖环”的报错由下一个用例（`the_cycle_report_...`）覆盖。
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let (a, b) = (g.memo(), g.memo());
            g.edge(a, b);
            g.edge(b, a);
            g.set_state(a, NodeState::Dirty);
            g.set_state(b, NodeState::Dirty);

            g.evaluate_node(b);
        });
    }

    #[test]
    fn the_cycle_report_names_every_node_on_the_cycle() {
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let (a, b, c) = (g.memo(), g.memo(), g.memo());
            g.edge(a, b);
            g.edge(b, c);
            g.edge(c, a);
            for id in [a, b, c] {
                g.set_state(id, NodeState::Dirty);
            }

            let frames = [EvalFrame::new(c), EvalFrame::new(b), EvalFrame::new(a)];
            let report = with_rt_or_init(|rt| rt.describe_cycle(&frames, c)).expect("运行时可用");

            for id in [a, b, c] {
                assert!(report.contains(&format!("#{}", id.slot())));
            }
        });
    }

    /// §2.2：一个有 k 个脏依赖的节点，它的依赖表只被增量扫过 —— 求值仍然
    /// 严格自上游向下游，而且每个依赖恰好跑一次。
    #[test]
    fn a_node_with_many_dirty_dependencies_evaluates_each_of_them_once() {
        on_a_fresh_runtime(|| {
            let g = Graph::new();
            let sink = g.memo();
            let deps: Vec<RawId> = (0..16).map(|_| g.memo()).collect();
            for &dep in &deps {
                g.edge(dep, sink);
                g.set_state(dep, NodeState::Dirty);
            }
            g.set_state(sink, NodeState::Dirty);

            g.evaluate_node(sink);

            let mut expected = deps.clone();
            expected.push(sink);
            assert_eq!(g.ran(), expected, "依赖按序各跑一次，最后才是它们的下游");
        });
    }
}
