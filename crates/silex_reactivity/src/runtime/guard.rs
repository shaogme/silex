//! 运行时状态的 RAII 守卫。
//!
//! 运行时的每一个“设标志 → 调用用户代码 → 恢复标志”的位置都必须用守卫来恢复。
//! 裸写法在用户代码 panic 时会让标志永远卡住：`running_queue` 卡在 `true` 会让
//! 整个响应式系统静默停摆，`batch_depth` 卡在非零会让所有更新被永久挂起，
//! 被 `take()` 出来的计算闭包则会永久丢失（AUDIT P2）。
//!
//! 另一类守卫（[`SignalValueGuard`] / [`PayloadGuard`] / [`NodeRunGuard`]）管的是
//! **借出**：把值或闭包整个移出节点再交给用户代码。阶段三之后这条纪律的收益
//! 更明确了 —— 节点里剩下的是一个 `None`，因此“闭包执行期间这个节点被销毁”
//! 只会让守卫在归还时查无此节点（值随守卫析构），而不会写进一块已经释放的槽位。

use crate::{
    ReactiveError, ReactiveResult,
    internal::{
        arena::Index as NodeId,
        value::{AnyValue, ThunkValue},
    },
    runtime::{Runtime, graph::EvalFrame, graph::NodeState, scheduler::WorkSpace},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    mem,
};

/// 恢复 `current_owner`（**所有权**：新节点挂在谁下面、`on_cleanup` 注册给谁）。
pub(crate) struct OwnerGuard<'a> {
    rt: &'a Runtime,
    prev: Option<NodeId>,
}

impl<'a> OwnerGuard<'a> {
    pub(crate) fn set(rt: &'a Runtime, owner: Option<NodeId>) -> Self {
        let prev = rt.current_owner();
        rt.set_owner(owner);
        Self { rt, prev }
    }
}

impl Drop for OwnerGuard<'_> {
    fn drop(&mut self) {
        self.rt.set_owner(self.prev);
    }
}

/// 恢复 `current_observer`（**依赖追踪**：读 signal 时把谁登记为订阅者）。
///
/// 与所有权是两件正交的事，所以是两个独立的变量：`untrack` 只清这一个，
/// 它里面创建的节点照样挂在当前 owner 下面（AUDIT 二轮 §1.1）。
pub(crate) struct ObserverGuard<'a> {
    rt: &'a Runtime,
    prev: Option<NodeId>,
}

impl<'a> ObserverGuard<'a> {
    pub(crate) fn set(rt: &'a Runtime, observer: Option<NodeId>) -> Self {
        let prev = rt.current_observer();
        rt.set_observer(observer);
        Self { rt, prev }
    }
}

impl Drop for ObserverGuard<'_> {
    fn drop(&mut self) {
        self.rt.set_observer(self.prev);
    }
}

/// 进入一个计算节点（effect / memo）的执行上下文。
///
/// 计算节点同时扮演两个角色：它是本次运行中新建节点的 **owner**，也是本次
/// 运行中读到的 signal 的 **observer**。这是**唯一**会把两者设成同一个 id 的地方。
pub(crate) struct ComputationGuard<'a> {
    _owner: OwnerGuard<'a>,
    _observer: ObserverGuard<'a>,
}

impl<'a> ComputationGuard<'a> {
    pub(crate) fn enter(rt: &'a Runtime, id: NodeId) -> Self {
        Self {
            _owner: OwnerGuard::set(rt, Some(id)),
            _observer: ObserverGuard::set(rt, Some(id)),
        }
    }
}

/// 维护一个嵌套深度计数（`batch_depth` / `evaluating`）。
pub(crate) struct DepthGuard<'a> {
    depth: &'a Cell<usize>,
    prev: usize,
}

impl<'a> DepthGuard<'a> {
    pub(crate) fn enter(depth: &'a Cell<usize>) -> Self {
        let prev = depth.get();
        depth.set(prev + 1);
        Self { depth, prev }
    }

    /// 本次进入是否是最外层的一次。
    pub(crate) fn is_outermost(&self) -> bool {
        self.prev == 0
    }
}

impl Drop for DepthGuard<'_> {
    fn drop(&mut self) {
        self.depth.set(self.prev);
    }
}

/// 占用调度器的 “正在执行队列” 标志。
///
/// [`QueueGuard::acquire`] 返回 `None` 表示外层已经在跑队列 ——
/// 此时调用方既不该重入执行，也不负责最终的 flush。
pub(crate) struct QueueGuard<'a>(&'a Cell<bool>);

impl<'a> QueueGuard<'a> {
    pub(crate) fn acquire(flag: &'a Cell<bool>) -> Option<Self> {
        if flag.get() {
            return None;
        }
        flag.set(true);
        Some(Self(flag))
    }
}

impl Drop for QueueGuard<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// 求值 DFS 的工作栈，析构时归还池子。
///
/// 之前是手写的 `borrow_vec` / `return_vec` 配对：`evaluate` 在依赖成环时会
/// panic，借出的容器就那样被丢弃了。只损失池化容量、不影响正确性，但与 lib.rs
/// 里“所有借出的东西都由 RAII 守卫恢复”的承诺不符（AUDIT P2 / 二轮 §2.5）。
///
/// 阶段三之后这里只剩一个容器：依赖表改成原地遍历，`temp_deps` 那个 `Vec`
/// 连同它的池子一起没有了（审计报告 §3.3）。
pub(crate) struct EvalStack<'a> {
    ws: &'a RefCell<WorkSpace>,
    stack: Vec<EvalFrame>,
}

impl<'a> EvalStack<'a> {
    pub(crate) fn acquire(ws: &'a RefCell<WorkSpace>) -> Self {
        let stack = ws.borrow_mut().borrow_stack();
        Self { ws, stack }
    }

    pub(crate) fn get(&mut self) -> &mut Vec<EvalFrame> {
        &mut self.stack
    }
}

impl Drop for EvalStack<'_> {
    fn drop(&mut self) {
        // 展开途中池子可能正被外层借着；这时宁可丢掉这点容量，也不能在
        // panic 里再 panic（那会直接 abort）。
        if let Ok(mut w) = self.ws.try_borrow_mut() {
            w.return_stack(mem::take(&mut self.stack));
        }
    }
}

/// 传播 BFS 的工作队列，析构时归还池子。理由同 [`EvalStack`]。
pub(crate) struct PropagateQueue<'a> {
    ws: &'a RefCell<WorkSpace>,
    queue: VecDeque<NodeId>,
}

impl<'a> PropagateQueue<'a> {
    pub(crate) fn acquire(ws: &'a RefCell<WorkSpace>) -> Self {
        let queue = ws.borrow_mut().borrow_deque();
        Self { ws, queue }
    }

    pub(crate) fn get(&mut self) -> &mut VecDeque<NodeId> {
        &mut self.queue
    }
}

impl Drop for PropagateQueue<'_> {
    fn drop(&mut self) {
        if let Ok(mut w) = self.ws.try_borrow_mut() {
            w.return_deque(mem::take(&mut self.queue));
        }
    }
}

/// 节点运行期间“借出”计算闭包。
///
/// 闭包必须先从节点里取出来才能在不持有节点载荷借用的情况下调用，
/// 但 `computation == None` 是一个语义上会导致静默破坏的中间态：本守卫保证
/// 它一定会被放回去（正常返回或 panic 展开都一样），同时清除重入标记。
pub(crate) struct NodeRunGuard<'a> {
    rt: &'a Runtime,
    id: NodeId,
    pub(crate) computation: Option<ThunkValue>,
}

impl<'a> NodeRunGuard<'a> {
    pub(crate) fn new(rt: &'a Runtime, id: NodeId, computation: Option<ThunkValue>) -> Self {
        Self {
            rt,
            id,
            computation,
        }
    }
}

impl Drop for NodeRunGuard<'_> {
    fn drop(&mut self) {
        // 被 panic 打断的运行留下的是一份**不完整**的依赖集合：`run_node` 在调用
        // 用户闭包之前就把状态置成了 `Clean`（AUDIT P8 的要求），闭包跑到一半
        // panic，节点却是“干净”的，而依赖表里只有 panic 点之前读到的那几条
        // （二轮 §2.5 第 2 条）。
        //
        // 对**惰性拉取**的节点（memo / derived，既有值又有计算）可以直接标回
        // `Dirty`：它们没有队列，下一次被读到时自然会重算一遍、把依赖补全。
        //
        // 对**推送调度**的 effect 则不能这么做。`propagate` 有一条优化 ——
        // 已经是 `Dirty` 的订阅者不再重复入队（“脏了就说明已经排过队了”）。
        // 一个 panic 之后停在 `Dirty` 却不在队列里的 effect 会命中这条短路，
        // 从此**再也不会被任何写入唤醒**，比留在 `Clean` 还糟。
        //
        // 那为什么不顺手把它重新入队？因为那等于“panic 的 effect 自动重试”：
        // 它会在紧接着的每一次 flush 里被重跑、再 panic 一次，把一个局部错误
        // 放大成每次写入都炸。所以这里保持现状，并把代价写在文档里：
        // 被 panic 打断的 effect 只对它**已经登记过**的依赖有反应，对 panic 点
        // 之后本该读到的那些没有。
        let interrupted = std::thread::panicking();
        let computation = self.computation.take();

        // 节点在自己的运行期间被销毁时走到这里：`computation` 随守卫一起析构。
        let Some(node) = self.rt.storage.node(self.id) else {
            return;
        };
        if interrupted && node.has_value() {
            node.state.set(NodeState::Dirty);
        }
        node.set_running(false);
        if let Some(f) = computation {
            node.effect.borrow_mut().computation = Some(f);
        }
    }
}

/// signal 值在用户闭包执行期间的“借出”状态。
///
/// 值被移出节点、放在守卫里交给用户闭包，节点里暂时是 `None`。
/// 这样运行时在用户代码执行期间不持有任何指向该节点载荷的借用（AUDIT P5）。
pub(crate) struct SignalValueGuard<'a> {
    rt: &'a Runtime,
    id: NodeId,
    value: Option<AnyValue>,
    /// 归还值的时候是否顺带把版本号递增掉。
    ///
    /// 版本号意味着“值真的变了”，只有写入落到值上之后才该递增（AUDIT P12）。
    bump_version: bool,
}

impl<'a> SignalValueGuard<'a> {
    pub(crate) fn new(rt: &'a Runtime, id: NodeId, value: AnyValue) -> Self {
        Self {
            rt,
            id,
            value: Some(value),
            bump_version: false,
        }
    }

    pub(crate) fn value_mut(&mut self) -> &mut AnyValue {
        self.value.as_mut().expect("signal value is borrowed out")
    }

    /// 借出期间只读地看一眼这个值（memo 重算时把旧值借给计算闭包用，AUDIT P9）。
    pub(crate) fn value(&self) -> Option<&AnyValue> {
        self.value.as_ref()
    }

    /// 声明“这次写入真的改了值”，值归还时一并递增版本号。
    pub(crate) fn bump_version_on_release(&mut self) {
        self.bump_version = true;
    }
}

impl Drop for SignalValueGuard<'_> {
    fn drop(&mut self) {
        // 节点在闭包执行期间被销毁时走到这里：值随守卫一起析构。
        let Some(node) = self.rt.storage.node(self.id) else {
            return;
        };
        if self.bump_version {
            node.bump_version();
        }
        if let Some(value) = self.value.take() {
            node.signal.borrow_mut().value = Some(value);
        }
    }
}

/// 非响应式载荷（stored value / callback / node-ref）在用户闭包执行期间的
/// “借出”状态 —— [`SignalValueGuard`] 那套纪律在 `extras` 表上的对应物。
///
/// 从前这条路径上根本没有守卫：`try_update_stored_value` 直接把
/// `SparseSecondaryMap::get_mut` 交出来的 `&mut AnyValue` 递给用户闭包。
/// 用户在闭包里读写**任何别的** stored value 或 signal 都会再动一次同一张表，
/// 在 Stacked Borrows 下这就作废了手里那个 `&mut` —— 而这是一段完全普通的用法：
///
/// ```ignore
/// store::try_update::<Config, _>(cfg, |c| {
///     c.theme = signal::try_get::<Theme>(theme).unwrap();  // ← 作废了 c
/// });                                                      // ← 之后用 c 即 UB
/// ```
///
/// 现在值在闭包执行期间被整个移出节点（节点里是 `None`），运行时不再持有任何
/// 指向该条目的借用；重入访问同一个节点会拿到
/// [`ReactiveError::Reentrant`] 而不是静默的 UB（审计报告 §2.1）。
pub(crate) struct PayloadGuard<'a> {
    rt: &'a Runtime,
    id: NodeId,
    value: Option<AnyValue>,
}

impl<'a> PayloadGuard<'a> {
    /// 把载荷移出节点，节点里换成 `None`。
    pub(crate) fn acquire(rt: &'a Runtime, id: NodeId) -> ReactiveResult<Self> {
        let slot = rt.storage.extras.get(id).ok_or(ReactiveError::NoSuchNode)?;
        // `try_borrow_mut` 而不是 `borrow_mut`：外层闭包正握着它时给一句
        // 明确的诊断，而不是一句 `already borrowed` 的 panic。
        let taken = slot
            .try_borrow_mut()
            .map_err(|_| ReactiveError::Reentrant)?
            .take()
            .ok_or(ReactiveError::Reentrant)?;
        Ok(Self {
            rt,
            id,
            value: Some(taken),
        })
    }

    pub(crate) fn value(&self) -> &AnyValue {
        self.value.as_ref().expect("payload is borrowed out")
    }

    pub(crate) fn value_mut(&mut self) -> &mut AnyValue {
        self.value.as_mut().expect("payload is borrowed out")
    }
}

impl Drop for PayloadGuard<'_> {
    fn drop(&mut self) {
        // 节点在闭包执行期间被销毁时走到这里：值随守卫一起析构。
        let Some(slot) = self.rt.storage.extras.get(self.id) else {
            return;
        };
        if let Some(value) = self.value.take() {
            *slot.borrow_mut() = Some(value);
        }
    }
}
