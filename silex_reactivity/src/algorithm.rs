use crate::arena::Index as NodeId;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeState {
    Clean,
    Check,
    Dirty,
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

    /// Queue a specific effect for later execution.
    fn queue_effect(&mut self, id: NodeId);

    /// Run the computation for the node.
    /// Should update the node's value and return true if it changed.
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
        if graph.is_effect(sub_id) {
            graph.queue_effect(sub_id);
        } else {
            let state = graph.get_state(sub_id);
            if state != NodeState::Dirty {
                graph.set_state(sub_id, NodeState::Dirty);
                queue.push_back(sub_id);
            }
        }
    }

    // BFS for downstream
    while let Some(current_id) = queue.pop_front() {
        temp_subs.clear();
        graph.fill_subscribers(current_id, temp_subs);

        for &sub_id in temp_subs.iter() {
            if graph.is_effect(sub_id) {
                graph.queue_effect(sub_id);
            } else {
                let state = graph.get_state(sub_id);
                // Optimization: Only propagate if Clean -> Check
                if state == NodeState::Clean {
                    graph.set_state(sub_id, NodeState::Check);
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

        if state == NodeState::Clean {
            stack.pop();
            continue;
        }

        // Step A: Check dependencies
        temp_deps.clear();
        graph.fill_dependencies(current, temp_deps);
        let mut found_non_clean = false;

        for &dep_id in temp_deps.iter() {
            if graph.get_state(dep_id) != NodeState::Clean {
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

        let mut needs_computation = true;

        if state == NodeState::Check {
            // Optimization: check versions
            if !graph.check_dependencies_changed(current) {
                needs_computation = false;
                graph.set_state(current, NodeState::Clean);
            }
        }

        if needs_computation {
            // If Dirty or (Check and changed), we run computation.
            // run_computation should update value and return if changed.
            let _changed = graph.run_computation(current);

            // After computation, strictly set to Clean
            graph.set_state(current, NodeState::Clean);
        }

        // Don't pop yet, let the next loop iteration see it's Clean and pop it.
        // This maintains the invariant that we only pop Clean nodes.
    }
}
