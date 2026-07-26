use crate::core::{
    algorithm::GraphScheduler,
    arena::{Index as NodeId, SparseSecondaryMap},
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
}

impl GraphScheduler for Scheduler {
    fn queue_effect(&self, id: NodeId) {
        if self.queued_observers.get(id).is_none() {
            self.queued_observers.insert(id, ());
            self.observer_queue.borrow_mut().push_back(id);
        }
    }
}

pub(crate) struct WorkSpace {
    pub(crate) vec_pool: Vec<Vec<NodeId>>,
    pub(crate) deque_pool: Vec<VecDeque<NodeId>>,
}

impl WorkSpace {
    pub(crate) fn new() -> Self {
        Self {
            vec_pool: Vec::new(),
            deque_pool: Vec::new(),
        }
    }

    pub(crate) fn borrow_vec(&mut self) -> Vec<NodeId> {
        self.vec_pool.pop().unwrap_or_default()
    }

    pub(crate) fn return_vec(&mut self, mut v: Vec<NodeId>) {
        v.clear();
        if self.vec_pool.len() < 32 {
            self.vec_pool.push(v);
        }
    }

    pub(crate) fn borrow_deque(&mut self) -> VecDeque<NodeId> {
        self.deque_pool.pop().unwrap_or_default()
    }

    pub(crate) fn return_deque(&mut self, mut d: VecDeque<NodeId>) {
        d.clear();
        if self.deque_pool.len() < 32 {
            self.deque_pool.push(d);
        }
    }
}
