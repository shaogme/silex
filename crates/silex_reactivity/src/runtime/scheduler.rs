//! 响应式系统的调度器与工作空间复用池。

use crate::{
    internal::arena::{RawId, SparseSecondaryMap},
    runtime::{graph::EvalFrame, guard::Depth},
};
use std::collections::VecDeque;

/// 单次调度或单次求值中允许的最大迭代计算次数。
///
/// 用于防止节点在运行期间循环更新自身依赖导致死循环冻结线程。
/// 触发该限制时系统将抛出包含具体节点位置的清晰 panic 信息。
/// 详见 [`crate::runtime::drive::run_queue`] 与 [`crate::runtime::drive::drive_eval`]。
pub(crate) const MAX_QUEUE_ITERATIONS: usize = 100_000;

/// 调度器状态，管理待执行的副作用队列与嵌套深度标记。
pub(crate) struct Scheduler {
    /// 工作空间对象池（用于复用求值栈与队列缓冲区）。
    pub(crate) workspace: WorkSpace,
    /// 待执行的副作用 (Observer/Effect) 节点 FIFO 双端队列。
    pub(crate) observer_queue: VecDeque<RawId>,
    /// 记录已入队副作用节点的稀疏集合（防止重复入队）。
    pub(crate) queued_observers: SparseSecondaryMap<()>,
    /// 标记当前是否正在消费执行副作用队列。
    pub(crate) running_queue: bool,
    /// 批量更新 (`batch`) 的嵌套深度计数。
    pub(crate) batch_depth: usize,
    /// 正在进行的惰性求值 DFS 的嵌套深度计数。
    ///
    /// DFS 求值期间暂停刷新副作用队列，确保在递归求值完成后统一刷新。
    pub(crate) evaluating: usize,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self {
            workspace: WorkSpace::new(),
            observer_queue: VecDeque::new(),
            queued_observers: SparseSecondaryMap::new(),
            running_queue: false,
            batch_depth: 0,
            evaluating: 0,
        }
    }

    /// 将一个副作用节点放入待执行队列中（若尚未入队）。
    pub(crate) fn queue_effect(&mut self, id: RawId) {
        if self.queued_observers.get(id).is_none() {
            self.queued_observers.insert(id, ());
            self.observer_queue.push_back(id);
        }
    }

    /// 获取指定嵌套深度的可变引用，供 [`DepthGuard`](crate::runtime::guard::DepthGuard) 使用。
    pub(crate) fn depth(&mut self, which: Depth) -> &mut usize {
        match which {
            Depth::Batch => &mut self.batch_depth,
            Depth::Evaluating => &mut self.evaluating,
        }
    }
}

/// 求值栈 (`Vec<EvalFrame>`) 与传播队列 (`VecDeque<RawId>`) 的对象复用池。
///
/// 在递归或重入求值时允许动态借用与归还容器，减少堆内存分配开销。
pub(crate) struct WorkSpace {
    stack_pool: Vec<Vec<EvalFrame>>,
    deque_pool: Vec<VecDeque<RawId>>,
}

/// 容器池保留的最大对象数量。
const MAX_POOLED: usize = 32;

impl WorkSpace {
    pub(crate) fn new() -> Self {
        Self {
            stack_pool: Vec::new(),
            deque_pool: Vec::new(),
        }
    }

    /// 借出一个求值栈缓冲区。
    pub(crate) fn borrow_stack(&mut self) -> Vec<EvalFrame> {
        self.stack_pool.pop().unwrap_or_default()
    }

    /// 归还一个求值栈缓冲区。
    pub(crate) fn return_stack(&mut self, mut stack: Vec<EvalFrame>) {
        stack.clear();
        if self.stack_pool.len() < MAX_POOLED {
            self.stack_pool.push(stack);
        }
    }

    /// 借出一个双端队列缓冲区。
    pub(crate) fn borrow_deque(&mut self) -> VecDeque<RawId> {
        self.deque_pool.pop().unwrap_or_default()
    }

    /// 归还一个双端队列缓冲区。
    pub(crate) fn return_deque(&mut self, mut deque: VecDeque<RawId>) {
        deque.clear();
        if self.deque_pool.len() < MAX_POOLED {
            self.deque_pool.push(deque);
        }
    }
}
