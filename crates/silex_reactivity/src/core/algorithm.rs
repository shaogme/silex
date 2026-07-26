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

/// Phase 2: Evaluation (Iterative DFS)
/// Updates the node if necessary by checking dependencies recursively.
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
