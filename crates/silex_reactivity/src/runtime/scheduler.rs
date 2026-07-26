use crate::{
    internal::arena::{Index as NodeId, SparseSecondaryMap},
    runtime::graph::EvalFrame,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

/// 单次 `run_queue` 允许执行的最大 effect 数。
///
/// 超过就意味着队列在自我喂养（两个 effect 互相写对方的依赖），继续跑下去只会
/// 冻死整个线程。上限的作用是把死循环换成一条带节点位置的报错（AUDIT P13）。
/// 取值参考 SolidJS 的同类保护（~1e5），远高于任何正常应用的一轮更新规模。
pub(crate) const MAX_QUEUE_ITERATIONS: usize = 100_000;

pub(crate) struct Scheduler {
    pub(crate) workspace: RefCell<WorkSpace>,
    pub(crate) observer_queue: RefCell<VecDeque<NodeId>>,
    pub(crate) queued_observers: SparseSecondaryMap<()>,
    pub(crate) running_queue: Cell<bool>,
    pub(crate) batch_depth: Cell<usize>,
    /// 正在进行的 `evaluate`（求值 DFS）嵌套深度。
    ///
    /// DFS 期间禁止 flush effect 队列：memo 重算会 `commit_update` → `notify_update`，
    /// 如果就地把整个队列跑完，effect 可能销毁节点，而 `evaluate` 的栈里还留着
    /// 这些 id（AUDIT P15）。改为等 DFS 结束后再统一 flush。
    pub(crate) evaluating: Cell<usize>,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self {
            workspace: RefCell::new(WorkSpace::new()),
            observer_queue: RefCell::new(VecDeque::new()),
            queued_observers: SparseSecondaryMap::new(),
            running_queue: Cell::new(false),
            batch_depth: Cell::new(0),
            evaluating: Cell::new(0),
        }
    }

    pub(crate) fn queue_effect(&self, id: NodeId) {
        if self.queued_observers.get(id).is_none() {
            self.queued_observers.insert(id, ());
            self.observer_queue.borrow_mut().push_back(id);
        }
    }
}

/// 求值栈与传播队列的复用池。
///
/// 这里曾经还有一个 `vec_pool: Vec<Vec<NodeId>>`，专门用来接
/// `fill_subscribers` / `fill_dependencies` 物化出来的订阅表和依赖表 ——
/// 一个纯粹由 `ReactiveGraph` 抽象层引入的问题，然后又用一层机制去缓解它
/// （审计报告 §3.3）。算法改成原地遍历之后，那两个 `Vec` 连同它的池子
/// 一起没有了；剩下的两个容器（DFS 的栈、BFS 的队列）是算法本身需要的。
///
/// 池子仍然必要，因为求值可以重入：memo 的计算闭包里再读一个脏 memo，
/// 就会在外层还没走完时开始新的一轮 DFS。
pub(crate) struct WorkSpace {
    stack_pool: Vec<Vec<EvalFrame>>,
    deque_pool: Vec<VecDeque<NodeId>>,
}

/// 每个池子最多留几个容器。嵌套深度在真实代码里是个位数。
const MAX_POOLED: usize = 32;

impl WorkSpace {
    pub(crate) fn new() -> Self {
        Self {
            stack_pool: Vec::new(),
            deque_pool: Vec::new(),
        }
    }

    pub(crate) fn borrow_stack(&mut self) -> Vec<EvalFrame> {
        self.stack_pool.pop().unwrap_or_default()
    }

    pub(crate) fn return_stack(&mut self, mut stack: Vec<EvalFrame>) {
        stack.clear();
        if self.stack_pool.len() < MAX_POOLED {
            self.stack_pool.push(stack);
        }
    }

    pub(crate) fn borrow_deque(&mut self) -> VecDeque<NodeId> {
        self.deque_pool.pop().unwrap_or_default()
    }

    pub(crate) fn return_deque(&mut self, mut deque: VecDeque<NodeId>) {
        deque.clear();
        if self.deque_pool.len() < MAX_POOLED {
            self.deque_pool.push(deque);
        }
    }
}
