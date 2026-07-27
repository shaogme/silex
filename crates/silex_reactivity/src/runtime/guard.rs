//! 运行时状态的 RAII 守卫。
//!
//! 运行时的每一个“设标志 → 调用用户代码 → 恢复标志”的位置都必须用守卫来恢复。
//! 裸写法在用户代码 panic 时会让标志永远卡住：`running_queue` 卡在 `true` 会让
//! 整个响应式系统静默停摆，`batch_depth` 卡在非零会让所有更新被永久挂起，
//! 被 `take()` 出来的计算闭包则会永久丢失（AUDIT P2）。
//!
//! 另一类守卫（[`SignalValueGuard`] / [`PayloadGuard`] / [`NodeRunGuard`]）管的是
//! **借出**：把值或闭包整个移出节点再交给用户代码。节点里剩下的是一个 `None`，
//! 因此“闭包执行期间这个节点被销毁”只会让守卫在归还时查无此节点（值随守卫
//! 析构），而不会写进一块已经释放的槽位。
//!
//! # 守卫为什么一个 `&Runtime` 都不拿
//!
//! 每一个守卫的存活期都**横跨用户代码**（这正是它存在的理由），而方案 B 之下
//! 用户代码只可能在两次 [`with_rt`] 之间执行 —— 借用出不了那个闭包。所以守卫
//! 只能存下恢复现场所需的**数据**（前一个 owner 是谁、借出的值本身、
//! 节点 id），用的时候现取一次借用。
//!
//! 析构里的那次 `with_rt` 用的是不会 panic 的形式：展开途中拿不到借用时宁可
//! 少恢复一次，也不能在 panic 里再 panic（那会直接 abort）。

use crate::{
    ReactiveError, ReactiveResult,
    internal::{
        arena::RawId,
        value::{AnyValue, Computation},
    },
    runtime::{Runtime, graph::EvalFrame, graph::NodeState, with_rt},
};
use std::mem;

/// 恢复 `current_owner`（**所有权**：新节点挂在谁下面、`on_cleanup` 注册给谁）。
pub(crate) struct OwnerGuard {
    prev: Option<RawId>,
}

impl OwnerGuard {
    /// 在一次已经拿到的借用里切换 owner。
    pub(crate) fn set(rt: &mut Runtime, owner: Option<RawId>) -> Self {
        let prev = rt.current_owner();
        rt.set_owner(owner);
        Self { prev }
    }
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        let _ = with_rt(|rt| rt.set_owner(self.prev));
    }
}

/// 恢复 `current_observer`（**依赖追踪**：读 signal 时把谁登记为订阅者）。
///
/// 与所有权是两件正交的事，所以是两个独立的变量：`untrack` 只清这一个，
/// 它里面创建的节点照样挂在当前 owner 下面（AUDIT 二轮 §1.1）。
pub(crate) struct ObserverGuard {
    prev: Option<RawId>,
}

impl ObserverGuard {
    pub(crate) fn set(rt: &mut Runtime, observer: Option<RawId>) -> Self {
        let prev = rt.current_observer();
        rt.set_observer(observer);
        Self { prev }
    }
}

impl Drop for ObserverGuard {
    fn drop(&mut self) {
        let _ = with_rt(|rt| rt.set_observer(self.prev));
    }
}

/// 进入一个计算节点（effect / memo）的执行上下文。
///
/// 计算节点同时扮演两个角色：它是本次运行中新建节点的 **owner**，也是本次
/// 运行中读到的 signal 的 **observer**。这是**唯一**会把两者设成同一个 id 的地方。
///
/// 它自己存两个 prev 而不是套一个 [`OwnerGuard`] 加一个 [`ObserverGuard`]：
/// 那样析构时要取**两次**借用，而这里两件事之间什么都没有。每一次 memo 重算、
/// 每一次 effect 执行都会付这一笔。
pub(crate) struct ComputationGuard {
    prev_owner: Option<RawId>,
    prev_observer: Option<RawId>,
    released: bool,
}

impl ComputationGuard {
    pub(crate) fn enter(rt: &mut Runtime, id: RawId) -> Self {
        let guard = Self {
            prev_owner: rt.current_owner(),
            prev_observer: rt.current_observer(),
            released: false,
        };
        rt.set_owner(Some(id));
        rt.set_observer(Some(id));
        guard
    }
}

impl ComputationGuard {
    /// 用一次**已经拿到的**借用提前退出，此后守卫是惰性的。
    pub(crate) fn release(&mut self, rt: &mut Runtime) {
        if self.released {
            return;
        }
        self.released = true;
        rt.set_owner(self.prev_owner);
        rt.set_observer(self.prev_observer);
    }
}

impl Drop for ComputationGuard {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let (owner, observer) = (self.prev_owner, self.prev_observer);
        let _ = with_rt(|rt| {
            rt.set_owner(owner);
            rt.set_observer(observer);
        });
    }
}

/// 调度器里的两个嵌套深度计数。
#[derive(Clone, Copy)]
pub(crate) enum Depth {
    /// `batch` 的嵌套层数：非零时所有 effect 只入队不执行。
    Batch,
    /// 正在进行的求值 DFS 的嵌套层数：非零时禁止 flush effect 队列（AUDIT P15）。
    Evaluating,
}

/// 维护一个嵌套深度计数。
pub(crate) struct DepthGuard {
    which: Depth,
    prev: usize,
}

impl DepthGuard {
    pub(crate) fn enter(which: Depth) -> Self {
        let prev = with_rt(|rt| {
            let depth = rt.scheduler.depth(which);
            let prev = *depth;
            *depth = prev + 1;
            prev
        })
        .unwrap_or(0);
        Self { which, prev }
    }

    /// 本次进入是否是最外层的一次。
    pub(crate) fn is_outermost(&self) -> bool {
        self.prev == 0
    }
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        let (which, prev) = (self.which, self.prev);
        let _ = with_rt(|rt| *rt.scheduler.depth(which) = prev);
    }
}

/// 占用调度器的 “正在执行队列” 标志。
///
/// [`QueueGuard::acquire`] 返回 `None` 表示外层已经在跑队列 ——
/// 此时调用方既不该重入执行，也不负责最终的 flush。
pub(crate) struct QueueGuard(());

impl QueueGuard {
    pub(crate) fn acquire() -> Option<Self> {
        let taken = with_rt(|rt| {
            if rt.scheduler.running_queue {
                return false;
            }
            rt.scheduler.running_queue = true;
            true
        })
        .unwrap_or(false);
        // `then` 而不是 `then_some`：后者会**先把 `Self(())` 造出来**再按条件
        // 丢弃，而丢弃一个 `QueueGuard` 就是跑一次它的 `Drop` —— 也就是在
        // “没抢到”的那条路径上，把**外层**持有的标志清成了 false。表现是队列
        // 执行开始互相嵌套，两个互相喂食的 effect 从“十万次之后报错”变成爆栈。
        taken.then(|| Self(()))
    }
}

impl Drop for QueueGuard {
    fn drop(&mut self) {
        let _ = with_rt(|rt| rt.scheduler.running_queue = false);
    }
}

/// 求值 DFS 的工作栈，析构时归还池子。
///
/// 之前是手写的 `borrow_vec` / `return_vec` 配对：`eval_step` 在依赖成环时会
/// panic，借出的容器就那样被丢弃了。只损失池化容量、不影响正确性，但与 lib.rs
/// 里“所有借出的东西都由 RAII 守卫恢复”的承诺不符（AUDIT P2 / 二轮 §2.5）。
///
/// 栈本身住在**驱动帧上**，不是运行时里的一块状态 —— 求值可以重入（memo 的
/// 计算闭包里再读一个脏 memo），每一层驱动各自持有自己的栈。
pub(crate) struct EvalStack {
    stack: Vec<EvalFrame>,
}

impl EvalStack {
    pub(crate) fn acquire(rt: &mut Runtime) -> Self {
        Self {
            stack: rt.scheduler.workspace.borrow_stack(),
        }
    }

    pub(crate) fn get(&mut self) -> &mut Vec<EvalFrame> {
        &mut self.stack
    }
}

impl Drop for EvalStack {
    fn drop(&mut self) {
        let stack = mem::take(&mut self.stack);
        // 展开途中拿不到借用时宁可丢掉这点容量，也不能在 panic 里再 panic。
        let _ = with_rt(|rt| rt.scheduler.workspace.return_stack(stack));
    }
}

// 传播 BFS 的工作队列这里曾经也有一个守卫。它没有了：传播整个跑在**一次
// 借用**之内（BFS 不执行一行用户代码，也没有 panic 路径），队列因此只是
// `queue_dependents` 里的一个局部变量，借出与归还紧挨着两行，中间没有任何
// 可能提前返回的东西。守卫在这里除了多一次 `with_rt` 什么也不提供。

/// 节点运行期间“借出”计算闭包。
///
/// 闭包必须先从节点里取出来才能在不持有任何运行时借用的情况下调用，
/// 但 `computation == None` 是一个语义上会导致静默破坏的中间态：本守卫保证
/// 它一定会被放回去（正常返回或 panic 展开都一样），同时清除重入标记。
pub(crate) struct NodeRunGuard {
    id: RawId,
    released: bool,
    pub(crate) computation: Option<Computation>,
}

impl NodeRunGuard {
    pub(crate) fn new(id: RawId, computation: Option<Computation>) -> Self {
        Self {
            id,
            released: false,
            computation,
        }
    }

    /// 用一次**已经拿到的**借用提前归还，此后守卫是惰性的。
    ///
    /// 正常收尾时它与 [`ComputationGuard::release`] 合成同一次借用 ——
    /// 每一次 memo 重算、每一次 effect 执行都会走这里，两次拆开写就是白付
    /// 一次借用。panic 展开时走的仍然是 `Drop`（那条路径要多做一件事：
    /// 把被打断的惰性节点标回 `Dirty`）。
    pub(crate) fn release(&mut self, rt: &mut Runtime) {
        self.released = true;
        // 节点在自己的运行期间被销毁了：闭包随守卫一起析构。
        let Some(node) = rt.storage.meta_mut(self.id) else {
            return;
        };
        node.set_running(false);
        if let Some(f) = self.computation.take()
            && let Some(computation) = rt.storage.computation_mut(self.id)
        {
            *computation = Some(f);
        }
    }
}

impl Drop for NodeRunGuard {
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
        if self.released {
            return;
        }
        let interrupted = std::thread::panicking();
        let computation = self.computation.take();
        let id = self.id;

        let _ = with_rt(|rt| {
            // 节点在自己的运行期间被销毁时走到这里：`computation` 随守卫一起析构。
            let Some(node) = rt.storage.meta_mut(id) else {
                return;
            };
            if interrupted && node.has_value() {
                node.state = NodeState::Dirty;
            }
            node.set_running(false);
            if let Some(f) = computation
                && let Some(slot) = rt.storage.computation_mut(id)
            {
                *slot = Some(f);
            }
        });
    }
}

/// signal 值在用户闭包执行期间的“借出”状态。
///
/// 值被移出节点、放在守卫里交给用户闭包，节点里暂时是 `None`。
/// 这样运行时在用户代码执行期间不持有任何指向该节点载荷的借用（AUDIT P5）。
pub(crate) struct SignalValueGuard {
    id: RawId,
    value: Option<AnyValue>,
    /// 归还值的时候是否顺带把版本号递增掉。
    ///
    /// 版本号意味着“值真的变了”，只有写入落到值上之后才该递增（AUDIT P12）。
    bump_version: bool,
}

impl SignalValueGuard {
    pub(crate) fn new(id: RawId, value: AnyValue) -> Self {
        Self {
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

    /// 用一次**已经拿到的**借用提前归还，此后守卫是惰性的。
    ///
    /// 写路径靠它把“归还值”和“失效下游”合成同一次 `with_rt`：用户闭包这时
    /// 已经跑完了，两件事之间没有任何会重入运行时的东西，分成两次借用纯粹是
    /// 白付一次线程本地查表。
    pub(crate) fn release(&mut self, rt: &mut Runtime) {
        if self.value.is_none() {
            return;
        }
        put_back(rt, self.id, self.value.take(), self.bump_version);
    }
}

/// 把借出的值放回节点。节点已经不在了就让值随之析构（它在调用方的栈上）。
fn put_back(rt: &mut Runtime, id: RawId, value: Option<AnyValue>, bump: bool) {
    if bump {
        let Some(node) = rt.storage.meta_mut(id) else {
            return;
        };
        node.bump_version();
    }
    if let Some(value) = value
        && let Some(slot) = rt.storage.value_mut(id)
    {
        *slot = Some(value);
    }
}

impl Drop for SignalValueGuard {
    fn drop(&mut self) {
        // 已经被 `release` 显式归还过了。
        if self.value.is_none() {
            return;
        }
        let (id, value, bump) = (self.id, self.value.take(), self.bump_version);
        // 节点在闭包执行期间被销毁时走到这里：值随守卫一起析构。
        let _ = with_rt(|rt| put_back(rt, id, value, bump));
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
pub(crate) struct PayloadGuard {
    id: RawId,
    value: Option<AnyValue>,
}

impl PayloadGuard {
    /// 把载荷移出节点，节点里换成 `None`。
    pub(crate) fn acquire(rt: &mut Runtime, id: RawId) -> ReactiveResult<Self> {
        let slot = rt
            .storage
            .extras
            .get_mut(id)
            .ok_or(ReactiveError::NoSuchNode)?;
        let taken = slot.take().ok_or(ReactiveError::Reentrant)?;
        Ok(Self {
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

impl Drop for PayloadGuard {
    fn drop(&mut self) {
        let value = self.value.take();
        let id = self.id;
        let _ = with_rt(|rt| {
            // 节点在闭包执行期间被销毁时走到这里：值随守卫一起析构。
            let Some(slot) = rt.storage.extras.get_mut(id) else {
                return;
            };
            if let Some(value) = value {
                *slot = Some(value);
            }
        });
    }
}
