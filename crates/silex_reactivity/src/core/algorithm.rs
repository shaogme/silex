use crate::core::arena::Index as NodeId;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeState {
    Clean,
    Check,
    Dirty,
}

/// Abstraction over the storage part of the reactive graph.
pub trait GraphStorage {
    fn get_state(&self, id: NodeId) -> NodeState;
    fn set_state(&self, id: NodeId, state: NodeState);
    fn fill_subscribers(&self, id: NodeId, dest: &mut Vec<NodeId>);
    fn fill_dependencies(&self, id: NodeId, dest: &mut Vec<NodeId>);
    fn is_effect(&self, id: NodeId) -> bool;
    fn is_running(&self, id: NodeId) -> bool;
    fn check_dependencies_changed(&self, id: NodeId) -> bool;
    /// 供诊断信息使用的人类可读描述（调试标签 + 定义位置）。
    fn describe(&self, id: NodeId) -> String;
}

/// Abstraction over the scheduler part of the reactive graph.
pub trait GraphScheduler {
    fn queue_effect(&self, id: NodeId);
}

/// Abstraction over the computation execution part (e.g. running user logic).
pub trait GraphExecutor {
    fn run_computation(&self, id: NodeId) -> bool;
}

/// A generic adapter that connects storage, scheduler, and executor to implement [ReactiveGraph].
pub struct RuntimeAdapter<'a, S, SCHED, E> {
    pub storage: &'a S,
    pub scheduler: &'a SCHED,
    pub executor: &'a E,
}

impl<S, SCHED, E> ReactiveGraph for RuntimeAdapter<'_, S, SCHED, E>
where
    S: GraphStorage,
    SCHED: GraphScheduler,
    E: GraphExecutor,
{
    fn get_state(&self, id: NodeId) -> NodeState {
        self.storage.get_state(id)
    }

    fn set_state(&mut self, id: NodeId, state: NodeState) {
        self.storage.set_state(id, state);
    }

    fn fill_subscribers(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        self.storage.fill_subscribers(id, dest);
    }

    fn fill_dependencies(&self, id: NodeId, dest: &mut Vec<NodeId>) {
        self.storage.fill_dependencies(id, dest);
    }

    fn is_effect(&self, id: NodeId) -> bool {
        self.storage.is_effect(id)
    }

    fn is_running(&self, id: NodeId) -> bool {
        self.storage.is_running(id)
    }

    fn queue_effect(&mut self, id: NodeId) {
        self.scheduler.queue_effect(id);
    }

    fn run_computation(&mut self, id: NodeId) -> bool {
        self.executor.run_computation(id)
    }

    fn check_dependencies_changed(&mut self, id: NodeId) -> bool {
        self.storage.check_dependencies_changed(id)
    }

    fn describe(&self, id: NodeId) -> String {
        self.storage.describe(id)
    }
}

/// Abstraction over the reactive graph to decouple algorithms from the runtime.
pub trait ReactiveGraph {
    /// Get the current state of a node.
    fn get_state(&self, id: NodeId) -> NodeState;

    /// Set the state of a node.
    fn set_state(&mut self, id: NodeId, state: NodeState);

    /// Fill the destination buffer with direct subscribers of a node.
    fn fill_subscribers(&self, id: NodeId, dest: &mut Vec<NodeId>);

    /// Fill the destination buffer with dependencies of a node.
    fn fill_dependencies(&self, id: NodeId, dest: &mut Vec<NodeId>);

    /// Check if a node is an effect (observer) that should be queued for execution.
    fn is_effect(&self, id: NodeId) -> bool;

    /// Check if a node's computation is currently executing.
    ///
    /// 正在运行的节点不能被重新求值（重入），也不应该被等待变干净 ——
    /// 它自身的重跑由调度队列负责。
    fn is_running(&self, id: NodeId) -> bool;

    /// Queue a specific effect for later execution.
    fn queue_effect(&mut self, id: NodeId);

    /// Run the computation for the node.
    ///
    /// 返回值表示**是否真的执行了计算闭包**（节点不存在或不是计算节点时为 false），
    /// 而不是“值有没有变化” —— 变更检测发生在 `commit_update` 里。
    ///
    /// 节点状态的所有权属于实现方：它必须在调用用户闭包**之前**把状态置为
    /// `Clean`，这样运行期间产生的失效标记才能保留下来（AUDIT P8）。
    fn run_computation(&mut self, id: NodeId) -> bool;

    /// Check if dependencies have changed versions relative to the last run.
    /// Optimizes Check -> Clean transition.
    fn check_dependencies_changed(&mut self, id: NodeId) -> bool;

    /// 人类可读的节点描述，只用于诊断信息（环检测的报错）。
    fn describe(&self, id: NodeId) -> String;
}

/// Phase 1: Propagation (BFS)
/// Marks downstream nodes as Dirty/Check and queues effects.
pub fn propagate(
    graph: &mut impl ReactiveGraph,
    start_node: NodeId,
    queue: &mut VecDeque<NodeId>,
    temp_subs: &mut Vec<NodeId>,
) {
    queue.clear();
    temp_subs.clear();

    // Initial: Mark start node's subscribers as Dirty
    graph.fill_subscribers(start_node, temp_subs);

    for &sub_id in temp_subs.iter() {
        let state = graph.get_state(sub_id);
        if state != NodeState::Dirty {
            graph.set_state(sub_id, NodeState::Dirty);
            if graph.is_effect(sub_id) {
                graph.queue_effect(sub_id);
            } else {
                queue.push_back(sub_id);
            }
        }
    }

    // BFS for downstream
    while let Some(current_id) = queue.pop_front() {
        temp_subs.clear();
        graph.fill_subscribers(current_id, temp_subs);

        for &sub_id in temp_subs.iter() {
            let state = graph.get_state(sub_id);
            // Optimization: Only propagate if Clean -> Check
            if state == NodeState::Clean {
                graph.set_state(sub_id, NodeState::Check);
                if graph.is_effect(sub_id) {
                    graph.queue_effect(sub_id);
                } else {
                    queue.push_back(sub_id);
                }
            }
        }
    }
}

/// 依赖环的诊断信息。
///
/// `stack` 是当前的 DFS 路径（栈顶是最近压入的节点），`repeated` 是又一次出现
/// 在这条路径上的节点。返回的字符串按“依赖方 → 被依赖方”的顺序列出这个环。
fn describe_cycle(graph: &impl ReactiveGraph, stack: &[NodeId], repeated: NodeId) -> String {
    let start = stack
        .iter()
        .position(|&id| id == repeated)
        .unwrap_or(stack.len());
    let mut chain: Vec<String> = stack[start..]
        .iter()
        .map(|&id| graph.describe(id))
        .collect();
    chain.push(graph.describe(repeated));
    chain.join("\n    -> 依赖 ")
}

/// Phase 2: Evaluation (Iterative DFS)
/// Updates the node if necessary by checking dependencies recursively.
///
/// # Panics
///
/// 依赖成环时 panic。`stack` 保存的是一条从目标节点出发的**简单路径**，
/// 一个节点第二次出现就意味着环 —— 之前没有任何检测，`A -> B -> A` 会让
/// 这个循环一直压栈直到 OOM（AUDIT P13）。
pub fn evaluate(
    graph: &mut impl ReactiveGraph,
    target_node: NodeId,
    stack: &mut Vec<NodeId>,
    temp_deps: &mut Vec<NodeId>,
) {
    if graph.get_state(target_node) == NodeState::Clean {
        return;
    }

    stack.clear();
    stack.push(target_node);

    while let Some(&current) = stack.last() {
        // Peek state
        let state = graph.get_state(current);

        // 正在运行中的节点不能在这里重新求值：它的重跑由调度队列负责。
        if state == NodeState::Clean || graph.is_running(current) {
            stack.pop();
            continue;
        }

        // Step A: Check dependencies
        temp_deps.clear();
        graph.fill_dependencies(current, temp_deps);
        let mut found_non_clean = false;

        for &dep_id in temp_deps.iter() {
            // 跳过正在运行的依赖：等它变干净会导致死循环（它不可能在本次 DFS 中变干净）。
            if graph.get_state(dep_id) != NodeState::Clean && !graph.is_running(dep_id) {
                // 这条 DFS 路径上已经有它了 —— 依赖成环。继续压栈只会让 `stack`
                // 一直长到 OOM，所以在这里带着环上的节点报错（AUDIT P13）。
                if stack.contains(&dep_id) {
                    panic!(
                        "silex_reactivity: 检测到依赖环，无法求值。环上的节点：\n    {}",
                        describe_cycle(graph, stack, dep_id)
                    );
                }
                stack.push(dep_id);
                found_non_clean = true;
                break; // DFS: Process dependency first
            }
        }

        if found_non_clean {
            continue; // Loop again to process the pushed dependency
        }

        // Step B: All dependencies are Clean (or we are at a leaf/signal).
        // Try to update current node.

        if state == NodeState::Check && !graph.check_dependencies_changed(current) {
            // Optimization: 版本号没变，无需重算。
            graph.set_state(current, NodeState::Clean);
            stack.pop();
            continue;
        }

        // If Dirty or (Check and changed), we run computation.
        // 状态转换由 run_computation 负责（运行前置 Clean）。这里**不能**再无条件
        // 写一次 Clean —— 那会把节点在自己运行期间产生的失效标记抹掉，
        // 使得队列里的重跑条目被当作“已干净”跳过，更新静默丢失（AUDIT P8）。
        if !graph.run_computation(current) {
            // 不是计算节点（例如已销毁节点残留的幽灵条目）：必须置 Clean，
            // 否则上游会反复把它压栈。
            graph.set_state(current, NodeState::Clean);
        }

        // 无论本次是否重新变脏都要出栈：重新变脏意味着它已经被重新入队，
        // 由调度队列负责重跑；留在栈上只会死循环。
        stack.pop();
    }
}

#[cfg(test)]
mod tests {
    //! 图算法脱离运行时的单元测试。
    //!
    //! 这一层本来就是按“可单独测试”设计的（`ReactiveGraph` 把存储、调度、执行
    //! 三件事都抽象掉了），但此前一个测试都没有（AUDIT P18）。

    use super::*;
    use crate::core::arena::Index;
    use std::collections::HashMap;

    fn id(n: u32) -> NodeId {
        Index {
            index: n,
            generation: 1,
        }
    }

    #[derive(Default)]
    struct TestNode {
        state: Option<NodeState>,
        subscribers: Vec<NodeId>,
        dependencies: Vec<NodeId>,
        is_effect: bool,
        running: bool,
        /// `check_dependencies_changed` 的返回值。
        deps_changed: bool,
        /// 为 false 时 `run_computation` 返回 false（模拟“不是计算节点”）。
        computes: bool,
    }

    #[derive(Default)]
    struct TestGraph {
        nodes: HashMap<u32, TestNode>,
        queued: Vec<NodeId>,
        ran: Vec<NodeId>,
    }

    impl TestGraph {
        fn node(&mut self, n: u32) -> &mut TestNode {
            self.nodes.entry(n).or_insert_with(|| TestNode {
                computes: true,
                ..TestNode::default()
            })
        }

        /// `from` 的变化会传播到 `to`（即 `to` 依赖 `from`）。
        fn edge(&mut self, from: u32, to: u32) {
            self.node(from).subscribers.push(id(to));
            self.node(to).dependencies.push(id(from));
        }
    }

    impl ReactiveGraph for TestGraph {
        fn get_state(&self, id: NodeId) -> NodeState {
            self.nodes
                .get(&id.index)
                .and_then(|n| n.state)
                .unwrap_or(NodeState::Clean)
        }

        fn set_state(&mut self, id: NodeId, state: NodeState) {
            // 与真实存储一致：不存在的节点直接忽略，绝不新建（AUDIT P14）。
            if let Some(n) = self.nodes.get_mut(&id.index) {
                n.state = Some(state);
            }
        }

        fn fill_subscribers(&self, id: NodeId, dest: &mut Vec<NodeId>) {
            if let Some(n) = self.nodes.get(&id.index) {
                dest.extend_from_slice(&n.subscribers);
            }
        }

        fn fill_dependencies(&self, id: NodeId, dest: &mut Vec<NodeId>) {
            if let Some(n) = self.nodes.get(&id.index) {
                dest.extend_from_slice(&n.dependencies);
            }
        }

        fn is_effect(&self, id: NodeId) -> bool {
            self.nodes.get(&id.index).is_some_and(|n| n.is_effect)
        }

        fn is_running(&self, id: NodeId) -> bool {
            self.nodes.get(&id.index).is_some_and(|n| n.running)
        }

        fn queue_effect(&mut self, id: NodeId) {
            self.queued.push(id);
        }

        fn run_computation(&mut self, id: NodeId) -> bool {
            let computes = self.nodes.get(&id.index).is_some_and(|n| n.computes);
            if !computes {
                return false;
            }
            // 契约：状态在跑闭包之前置 Clean（AUDIT P8）。
            self.set_state(id, NodeState::Clean);
            self.ran.push(id);
            true
        }

        fn check_dependencies_changed(&mut self, id: NodeId) -> bool {
            self.nodes.get(&id.index).is_some_and(|n| n.deps_changed)
        }

        fn describe(&self, id: NodeId) -> String {
            format!("#{}", id.index)
        }
    }

    fn propagate_from(graph: &mut TestGraph, start: u32) {
        let mut queue = VecDeque::new();
        let mut subs = Vec::new();
        propagate(graph, id(start), &mut queue, &mut subs);
    }

    fn evaluate_node(graph: &mut TestGraph, target: u32) {
        let mut stack = Vec::new();
        let mut deps = Vec::new();
        evaluate(graph, id(target), &mut stack, &mut deps);
    }

    // --- propagate ---

    #[test]
    fn propagate_marks_direct_subscribers_dirty_and_the_rest_check() {
        // 0 -> 1 -> 2
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.edge(1, 2);

        propagate_from(&mut g, 0);

        assert_eq!(g.get_state(id(1)), NodeState::Dirty, "直接订阅者是 Dirty");
        assert_eq!(g.get_state(id(2)), NodeState::Check, "间接下游只是 Check");
    }

    #[test]
    fn propagate_queues_effects_instead_of_walking_past_them() {
        // 0 -> 1(effect) -> 2
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.edge(1, 2);
        g.node(1).is_effect = true;

        propagate_from(&mut g, 0);

        assert_eq!(g.queued, vec![id(1)]);
        assert_eq!(
            g.get_state(id(2)),
            NodeState::Clean,
            "effect 没有订阅者语义上的下游，传播到它为止"
        );
    }

    #[test]
    fn propagate_does_not_downgrade_a_dirty_node_to_check() {
        // 0 -> 2, 0 -> 1 -> 2：菱形的短边先把 2 标 Dirty，长边不能把它降成 Check
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.edge(0, 2);
        g.edge(1, 2);

        propagate_from(&mut g, 0);

        assert_eq!(g.get_state(id(2)), NodeState::Dirty);
    }

    #[test]
    fn propagate_ignores_nodes_that_no_longer_exist() {
        let mut g = TestGraph::default();
        g.node(0).subscribers.push(id(7)); // 7 已被销毁

        propagate_from(&mut g, 0);

        assert!(
            !g.nodes.contains_key(&7),
            "不得为已销毁的订阅者造出幽灵条目（AUDIT P14）"
        );
    }

    // --- evaluate ---

    #[test]
    fn evaluating_a_clean_node_does_nothing() {
        let mut g = TestGraph::default();
        g.node(0);

        evaluate_node(&mut g, 0);

        assert!(g.ran.is_empty());
    }

    #[test]
    fn evaluate_runs_dependencies_before_dependents() {
        // 0 -> 1 -> 2，全部标脏后从 2 开始求值
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.edge(1, 2);
        for n in 0..3 {
            g.node(n).state = Some(NodeState::Dirty);
        }

        evaluate_node(&mut g, 2);

        assert_eq!(g.ran, vec![id(0), id(1), id(2)], "必须自上游向下游依次求值");
    }

    #[test]
    fn a_check_node_whose_dependencies_did_not_change_is_not_recomputed() {
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.node(1).state = Some(NodeState::Check);
        g.node(1).deps_changed = false;

        evaluate_node(&mut g, 1);

        assert!(g.ran.is_empty(), "版本号没变就不该重算");
        assert_eq!(g.get_state(id(1)), NodeState::Clean);
    }

    #[test]
    fn a_check_node_whose_dependencies_changed_is_recomputed() {
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.node(1).state = Some(NodeState::Check);
        g.node(1).deps_changed = true;

        evaluate_node(&mut g, 1);

        assert_eq!(g.ran, vec![id(1)]);
    }

    #[test]
    fn a_running_node_is_never_re_evaluated() {
        let mut g = TestGraph::default();
        g.node(0).state = Some(NodeState::Dirty);
        g.node(0).running = true;

        evaluate_node(&mut g, 0);

        assert!(g.ran.is_empty(), "正在运行的节点重跑由队列负责（AUDIT P1）");
        assert_eq!(
            g.get_state(id(0)),
            NodeState::Dirty,
            "它的失效标记必须保留，否则队列里的重跑条目会被当成已干净跳过"
        );
    }

    #[test]
    fn a_running_dependency_does_not_block_its_dependent() {
        // 1 依赖 0，0 正在运行：不能停下来等它变干净（它不可能在本次 DFS 里变干净）
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.node(0).state = Some(NodeState::Dirty);
        g.node(0).running = true;
        g.node(1).state = Some(NodeState::Dirty);

        evaluate_node(&mut g, 1);

        assert_eq!(g.ran, vec![id(1)]);
    }

    #[test]
    fn a_non_computation_node_is_forced_clean() {
        // 幽灵条目：有状态但跑不了计算，必须被置 Clean，否则上游会反复压栈
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.node(0).state = Some(NodeState::Dirty);
        g.node(0).computes = false;
        g.node(1).state = Some(NodeState::Dirty);

        evaluate_node(&mut g, 1);

        assert_eq!(g.get_state(id(0)), NodeState::Clean);
        assert_eq!(g.ran, vec![id(1)]);
    }

    #[test]
    #[should_panic(expected = "依赖环")]
    fn a_dependency_cycle_panics() {
        // 0 <-> 1
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.edge(1, 0);
        g.node(0).state = Some(NodeState::Dirty);
        g.node(1).state = Some(NodeState::Dirty);

        evaluate_node(&mut g, 1);
    }

    #[test]
    fn the_cycle_report_names_every_node_on_the_cycle() {
        let mut g = TestGraph::default();
        g.edge(0, 1);
        g.edge(1, 2);
        g.edge(2, 0);
        for n in 0..3 {
            g.node(n).state = Some(NodeState::Dirty);
        }

        let report = describe_cycle(&g, &[id(2), id(1), id(0)], id(2));

        assert!(report.contains("#0") && report.contains("#1") && report.contains("#2"));
    }
}
