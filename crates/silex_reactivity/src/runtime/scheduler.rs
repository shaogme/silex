use crate::{
    internal::arena::{Index as NodeId, SparseSecondaryMap},
    runtime::{graph::EvalFrame, guard::Depth},
};
use std::collections::VecDeque;

/// 单次调度允许执行的最大计算次数。
///
/// 超过就意味着有东西在自我喂养 —— 两个 effect 互相写对方的依赖，或者一个
/// 节点在自己的运行过程中写回自己的上游。继续跑下去只会冻死整个线程。
/// 上限的作用是把死循环换成一条带节点位置的报错（AUDIT P13）。
/// 取值参考 SolidJS 的同类保护（~1e5），远高于任何正常应用的一轮更新规模。
///
/// 两条路径各自计数：effect 队列见 [`crate::runtime::drive::run_queue`]，
/// 求值 DFS 见 [`crate::runtime::drive::drive_eval`]。P13 当初只盖住了前者，
/// 而后者同样会不收敛且**一次都碰不到队列的计数器**。
pub(crate) const MAX_QUEUE_ITERATIONS: usize = 100_000;

/// 调度器的字段现在全是**普通字段**。
///
/// 从前它们是 `Cell` / `RefCell`：`RUNTIME.get()` 只交出 `&Runtime`，想改任何
/// 东西都得靠内部可变性。访问入口收成 `&mut Runtime`（[`with_rt`](crate::runtime::with_rt)）
/// 之后这一层全部消失，热路径上的借用计数也随之归零。
pub(crate) struct Scheduler {
    pub(crate) workspace: WorkSpace,
    pub(crate) observer_queue: VecDeque<NodeId>,
    pub(crate) queued_observers: SparseSecondaryMap<()>,
    pub(crate) running_queue: bool,
    pub(crate) batch_depth: usize,
    /// 正在进行的求值 DFS 的嵌套深度。
    ///
    /// DFS 期间禁止 flush effect 队列：memo 重算会 `commit_update` → `notify_update`，
    /// 如果就地把整个队列跑完，effect 可能销毁节点，而求值栈里还留着这些 id
    /// （AUDIT P15）。改为等 DFS 结束后再统一 flush。
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

    pub(crate) fn queue_effect(&mut self, id: NodeId) {
        if self.queued_observers.get(id).is_none() {
            self.queued_observers.insert(id, ());
            self.observer_queue.push_back(id);
        }
    }

    /// 两个嵌套深度计数的统一入口，供 [`DepthGuard`](crate::runtime::guard::DepthGuard) 用。
    pub(crate) fn depth(&mut self, which: Depth) -> &mut usize {
        match which {
            Depth::Batch => &mut self.batch_depth,
            Depth::Evaluating => &mut self.evaluating,
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
