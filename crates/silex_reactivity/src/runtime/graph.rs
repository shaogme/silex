//! 图算法：传播（BFS）与求值（DFS）。
//!
//! # 这里为什么不再有抽象层
//!
//! 从前这两个算法住在 `core::algorithm` 里，隔着四个 trait
//! （`GraphStorage` / `GraphScheduler` / `GraphExecutor` / `ReactiveGraph`）
//! 加一个 `RuntimeAdapter`，一共约 120 行纯转发代码。生产实现者只有一个，
//! 抽象的**唯一**收益是让底部那批单元测试能用一个 `TestGraph` 跑（AUDIT P18）。
//!
//! 代价却是实打实的：`fill_subscribers(&self, id, dest: &mut Vec<NodeId>)` 这个
//! 签名**强制把订阅表和依赖表物化进一个 `Vec`**（trait 没法表达“借用内部的
//! `List<NodeId>`”），于是 BFS 每访问一个节点就要整表拷贝一次，还得配一套
//! `vec_pool` / `deque_pool` 去缓解这个由抽象自己引入的问题（审计报告 §3.3）。
//!
//! 阶段三把 `ReactiveNode` 的元数据换成 `Cell` 之后，“持有一个节点的引用去改
//! 另一个节点的状态”成了合法操作，原地遍历才第一次成为可能。于是：
//!
//! - 两个算法变成 [`Runtime`] 上的方法，直接在 `List<NodeId>` 上走；
//! - `Vec` 物化与 `vec_pool` 一并删除（求值 DFS 的工作栈仍然池化）；
//! - `evaluate` 的依赖扫描带上游标，不再对同一个节点反复全量重扫（§2.2）；
//! - 那批单元测试改在**真实的** `Runtime` 上搭图 —— 顺带把“真实存储与测试
//!   替身行为一致”从假设变成了事实。

use crate::{
    internal::arena::Index as NodeId,
    runtime::{
        Runtime,
        scheduler::Scheduler,
        storage::{ReactiveNode, Storage},
    },
};
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NodeState {
    Clean,
    Check,
    Dirty,
}

/// [`Runtime::eval_step`] 交还给驱动循环的东西。
///
/// 求值 DFS 本身不跑一行用户代码；每当它走到“该跑一次计算了”，就把节点 id
/// 交还给驱动，由驱动在不持有任何运行时内部借用的情况下执行。
#[derive(Clone, Copy)]
pub(crate) enum Step {
    /// 本次 DFS 走完了。
    Done,
    /// 该跑这个节点的计算了。
    Run(NodeId),
}

/// 求值 DFS 的一帧。
///
/// `cursor` 是这个节点的依赖表里下一个待检查的下标 —— §2.2 的修复：从前每次
/// 回到同一个节点都要把整张依赖表重新填进一个 `Vec` 再从头扫一遍，一个有 k 个
/// 脏依赖的节点会被扫 k+1 遍，总代价 O(k²) 外加 k+1 次整表拷贝。
#[derive(Clone, Copy)]
pub(crate) struct EvalFrame {
    node: NodeId,
    cursor: usize,
}

impl EvalFrame {
    #[inline]
    pub(crate) fn new(node: NodeId) -> Self {
        Self { node, cursor: 0 }
    }
}

impl Runtime {
    // --- 就地遍历 ---

    /// 就地遍历一个节点的订阅者表。
    ///
    /// 取 `&Storage` 而不是 `&self`：调用方要一边遍历订阅者表、一边改**别的**
    /// 节点的状态和调度器，而借用检查器只看得见整个 `self`。把参数收窄到真正
    /// 用到的那张表，调用方就能显式解构出两个不相交的字段借用（见
    /// [`Runtime::propagate`]）。
    ///
    /// # `f` 里能做什么
    ///
    /// 遍历期间本节点的 signal 载荷处于**共享借用**状态，因此 `f` 不得改动
    /// **本节点自己**的订阅者表。传播只写 `Cell` 元数据、effect 队列与调度器
    /// 的旁路表，而且一个节点不可能订阅自己（`track_dependency` 在
    /// `observer == target` 时直接返回），这条约束是被结构性满足的。
    #[inline]
    fn for_each_subscriber(storage: &Storage, id: NodeId, mut f: impl FnMut(NodeId)) {
        let Some(node) = storage.node(id) else {
            return;
        };
        let slot = node.signal.borrow();
        for &sub_id in slot.subscribers.as_slice() {
            f(sub_id);
        }
    }

    /// 从下标 `from` 起，找 `id` 的依赖表里第一个既不干净、又不在运行中的依赖。
    ///
    /// 返回它以及它在依赖表里的下标。跳过正在运行的依赖：等它变干净会死循环
    /// （它不可能在本次 DFS 中变干净）。
    fn next_unclean_dependency(&self, id: NodeId, from: usize) -> Option<(NodeId, usize)> {
        let node = self.storage.node(id)?;
        let slot = node.effect.borrow();
        let deps = slot.dependencies.as_slice();
        let start = from.min(deps.len());
        for (offset, &(dep_id, _)) in deps.iter().enumerate().skip(start) {
            if self.storage.get_state(dep_id) != NodeState::Clean
                && !self.storage.is_running(dep_id)
            {
                return Some((dep_id, offset));
            }
        }
        None
    }

    /// 依赖的版本号相对上一次运行有没有变过 —— `Check -> Clean` 的快路径。
    fn dependencies_changed(&self, id: NodeId) -> bool {
        let Some(node) = self.storage.node(id) else {
            return false;
        };
        let slot = node.effect.borrow();
        slot.dependencies
            .as_slice()
            .iter()
            .any(|&(dep_id, expected)| match self.storage.node(dep_id) {
                Some(dep) if dep.has_value() => dep.version.get() != expected,
                // 依赖已经没了（或者根本没有值）：一律当作变了。
                _ => true,
            })
    }

    // --- Phase 1: 传播（BFS） ---

    /// 把下游标记为 `Dirty` / `Check`，并把其中的 effect 推进队列。
    pub(crate) fn propagate(&mut self, start_node: NodeId, queue: &mut VecDeque<NodeId>) {
        queue.clear();

        // 显式解构出两个**不相交**的字段借用：订阅者表来自 `storage`（共享借用
        // 就够，状态是 `Cell`），而入队要改 `scheduler`。少了这一步，
        // “借着 A 的订阅者表去改 B 的状态”根本写不出来 —— 借用检查器只看得见
        // 整个 `self`（设计文档 §4.3）。
        let Runtime {
            storage, scheduler, ..
        } = self;

        // 直接订阅者一律 Dirty。
        Self::for_each_subscriber(storage, start_node, |sub_id| {
            if storage.get_state(sub_id) != NodeState::Dirty {
                storage.set_state(sub_id, NodeState::Dirty);
                Self::schedule_or_walk(storage, scheduler, sub_id, queue);
            }
        });

        while let Some(current_id) = queue.pop_front() {
            Self::for_each_subscriber(storage, current_id, |sub_id| {
                // 只把 Clean 提升到 Check —— 绝不把已经 Dirty 的节点降级。
                if storage.get_state(sub_id) == NodeState::Clean {
                    storage.set_state(sub_id, NodeState::Check);
                    Self::schedule_or_walk(storage, scheduler, sub_id, queue);
                }
            });
        }
    }

    /// effect 进队列（它没有订阅者语义上的下游，传播到它为止），其余继续走 BFS。
    #[inline]
    fn schedule_or_walk(
        storage: &Storage,
        scheduler: &mut Scheduler,
        id: NodeId,
        queue: &mut VecDeque<NodeId>,
    ) {
        if storage.node(id).is_some_and(ReactiveNode::is_effect) {
            scheduler.queue_effect(id);
        } else {
            queue.push_back(id);
        }
    }

    // --- Phase 2: 求值（迭代式 DFS） ---

    /// 必要时沿依赖向上把一个节点算干净。
    ///
    /// 这是求值的**驱动循环**：DFS 本身由 [`Runtime::eval_step`] 一步步推进，
    /// 而每当它走到“该跑一次计算了”，控制权就回到这里，由驱动在**不持有任何
    /// 运行时内部借用**的情况下调用 [`Runtime::run_node`]。
    ///
    /// 工作栈是驱动帧上的一个局部 `Vec`（从池子里借出来的），不是运行时里的
    /// 一块状态 —— 这一点是整个设计成立的关键：求值可以重入（memo 的计算闭包
    /// 里再读一个脏 memo），每一层驱动各自持有自己的栈，互不干扰。方案 B 之下
    /// `eval_step` 会变成一次 `with_rt`，而 `run_node` 落在两次借用之间。
    ///
    /// 推进 DFS，直到走完（[`Step::Done`]）或撞上一次需要执行用户代码的计算
    /// （[`Step::Run`]）。
    ///
    /// 本方法自己**不执行任何用户代码** —— 这正是它与从前那个一路跑到底的
    /// `evaluate` 的全部区别，也是它能整个跑在一次借用之内的原因。
    /// 驱动循环见 [`drive_eval`](crate::runtime::drive::drive_eval)。
    ///
    /// # Panics
    ///
    /// 依赖成环时 panic。`stack` 保存的是一条从目标节点出发的**简单路径**，
    /// 一个节点第二次出现就意味着环 —— 之前没有任何检测，`A -> B -> A` 会让
    /// 这个循环一直压栈直到 OOM（AUDIT P13）。
    pub(crate) fn eval_step(&self, stack: &mut Vec<EvalFrame>) -> Step {
        while let Some(&EvalFrame {
            node: current,
            cursor,
        }) = stack.last()
        {
            let state = self.storage.get_state(current);

            // 正在运行中的节点不能在这里重新求值：它的重跑由调度队列负责。
            if state == NodeState::Clean || self.storage.is_running(current) {
                stack.pop();
                continue;
            }

            // Step A：找一个还不干净的依赖，先把它算掉。
            //
            // 增量扫描扫到末尾之后再从头全量复查一遍：DFS 期间可能有用户代码
            // 重入运行时、把一个已经扫过的依赖重新标脏。全量复查每个节点至多
            // 发生一次，因此总代价仍是 O(k)，而行为与“每次都全量重扫”等价。
            let found = match self.next_unclean_dependency(current, cursor) {
                hit @ Some(_) => hit,
                None if cursor > 0 => self.next_unclean_dependency(current, 0),
                None => None,
            };

            if let Some((dep_id, at)) = found {
                // 这条 DFS 路径上已经有它了 —— 依赖成环。继续压栈只会让 `stack`
                // 一直长到 OOM，所以在这里带着环上的节点报错（AUDIT P13）。
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

            // Step B：依赖全都干净了（或者本来就是叶子），轮到自己。

            if state == NodeState::Check && !self.dependencies_changed(current) {
                // 版本号没变，无需重算。
                self.storage.set_state(current, NodeState::Clean);
                stack.pop();
                continue;
            }

            // Dirty，或者 Check 且依赖确实变了：把控制权交还给驱动。
            return Step::Run(current);
        }

        Step::Done
    }

    /// 依赖环的诊断信息。
    ///
    /// `stack` 是当前的 DFS 路径（栈顶是最近压入的节点），`repeated` 是又一次
    /// 出现在这条路径上的节点。返回的字符串按“依赖方 → 被依赖方”的顺序列出
    /// 这个环。
    fn describe_cycle(&self, stack: &[EvalFrame], repeated: NodeId) -> String {
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
            storage::{EffectSlot, NodeFlags, ReactiveNode, SignalSlot},
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
        ran: Rc<RefCell<Vec<NodeId>>>,
    }

    impl Graph {
        fn new() -> Self {
            Self {
                ran: Rc::new(RefCell::new(Vec::new())),
            }
        }

        /// 只有值、没有计算：一个普通 signal。
        fn signal(&self) -> NodeId {
            drive::create_signal(AnyValue::new(0i32)).expect("运行时可用")
        }

        /// 计算闭包只记录“我被跑过了”。
        ///
        /// 带值的节点（memo）必须装一个 `MemoThunk` —— 它的返回值会被
        /// `recompute_memo` 提交进节点，因此这里恒返回同一个常量：
        /// 首算时没有旧值，一律算“变了”；之后每次都与旧值相等，不再惊动下游。
        /// 用例只看 `ran`，这个选择让它们不受提交语义干扰。
        fn computation(&self, flags: NodeFlags) -> NodeId {
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
                rt.storage.reactive.insert(
                    id,
                    ReactiveNode::new(
                        NodeState::Clean,
                        flags,
                        SignalSlot::default(),
                        EffectSlot {
                            computation: Some(thunk),
                            dependencies: Default::default(),
                        },
                    ),
                );
                id
            })
            .expect("运行时可用")
        }

        /// 既有值又有计算：memo / derived。
        fn memo(&self) -> NodeId {
            self.computation(NodeFlags::VALUE.with(NodeFlags::COMPUTATION))
        }

        /// 只有计算：effect。
        fn effect(&self) -> NodeId {
            self.computation(NodeFlags::COMPUTATION)
        }

        /// `from` 的变化会传播到 `to`（即 `to` 依赖 `from`）。
        fn edge(&self, from: NodeId, to: NodeId) {
            with_rt_or_init(|rt| {
                let version = rt.storage.node(from).map_or(0, |n| n.version.get());
                rt.storage
                    .node(from)
                    .expect("from 必须存在")
                    .signal
                    .borrow_mut()
                    .subscribers
                    .push(to);
                rt.storage
                    .node(to)
                    .expect("to 必须存在")
                    .effect
                    .borrow_mut()
                    .dependencies
                    .push((from, version));
            })
            .expect("运行时可用");
        }

        fn state(&self, id: NodeId) -> NodeState {
            with_rt_or_init(|rt| rt.storage.get_state(id)).expect("运行时可用")
        }

        fn set_state(&self, id: NodeId, state: NodeState) {
            with_rt_or_init(|rt| rt.storage.set_state(id, state)).expect("运行时可用");
        }

        fn set_running(&self, id: NodeId, running: bool) {
            with_rt_or_init(|rt| {
                rt.storage
                    .node(id)
                    .expect("节点必须存在")
                    .set_running(running);
            })
            .expect("运行时可用");
        }

        fn alive(&self, id: NodeId) -> bool {
            with_rt_or_init(|rt| rt.storage.node(id).is_some()).expect("运行时可用")
        }

        fn ran(&self) -> Vec<NodeId> {
            self.ran.borrow().clone()
        }

        fn propagate_from(&self, start: NodeId) -> Vec<NodeId> {
            with_rt_or_init(|rt| {
                let mut queue = VecDeque::new();
                rt.propagate(start, &mut queue);
                rt.scheduler.observer_queue.iter().copied().collect()
            })
            .expect("运行时可用")
        }

        fn evaluate_node(&self, target: NodeId) {
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
            drive::dispose(dead);

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
            with_rt_or_init(|rt| rt.storage.node(a).expect("a 活着").bump_version())
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
            let deps: Vec<NodeId> = (0..16).map(|_| g.memo()).collect();
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
