//! 运行时状态与资源借出的 RAII 守卫。
//!
//! RAII 守卫负责管理两类生命周期：
//! 1. **标志与上下文恢复**：确保在用户闭包成功返回或 panic 展开时，正确的 Owner/Observer、嵌套深度与运行标志能自动恢复。
//! 2. **资源移出与重入安全**：在执行用户闭包前将节点中的值或闭包移出（存储置为 `None`），避免用户代码执行期间运行时持有指向节点载荷的借用，从而彻底杜绝别名指针与 UB 风险。

use crate::{
    ReactiveError, ReactiveResult,
    internal::{
        arena::RawId,
        value::{AnyValue, Computation},
    },
    runtime::{Runtime, graph::EvalFrame, graph::NodeState, with_rt},
};
use std::mem;

/// 所有权上下文守卫，析构时自动恢复之前的父节点 `current_owner`。
pub(crate) struct OwnerGuard {
    prev: Option<RawId>,
}

impl OwnerGuard {
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

/// 依赖追踪观察者守卫，析构时自动恢复之前的 `current_observer`。
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

/// 进入计算节点（Effect / Memo）执行上下文的组合守卫。
///
/// 同时接管 `current_owner` 与 `current_observer` 的设置与恢复。
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
    /// 在已知包含 Runtime 借用的作用域内显式释放守卫。
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

/// 调度器嵌套深度类型。
#[derive(Clone, Copy)]
pub(crate) enum Depth {
    /// `batch` 批量更新嵌套深度。
    Batch,
    /// 惰性求值 DFS 的嵌套深度。
    Evaluating,
}

/// 维护调度器指定深度计数的 RAII 守卫。
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

    /// 判断本次进入是否为最外层（即之前的深度计数为 0）。
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

/// 副作用队列执行锁守卫，保证同一时刻只有一个队列消费循环在运行。
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
        taken.then(|| Self(()))
    }
}

impl Drop for QueueGuard {
    fn drop(&mut self) {
        let _ = with_rt(|rt| rt.scheduler.running_queue = false);
    }
}

/// 求值 DFS 工作栈守卫，在析构时将借出的 `Vec<EvalFrame>` 安全归还给对象池。
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
        let _ = with_rt(|rt| rt.scheduler.workspace.return_stack(stack));
    }
}

/// 节点运行期间借出计算闭包的守卫。
///
/// 保证计算闭包在使用完或 panic 时安全归还给节点，同时清空运行标记。
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

    /// 在已知包含 Runtime 借用的作用域内显式归还闭包并清理状态。
    pub(crate) fn release(&mut self, rt: &mut Runtime) {
        self.released = true;
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
        if self.released {
            return;
        }
        let interrupted = std::thread::panicking();
        let computation = self.computation.take();
        let id = self.id;

        let _ = with_rt(|rt| {
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

/// Signal 响应式值在用户闭包运行期间的借出守卫。
///
/// 将值临时移出节点，在闭包执行结束后根据 `bump_version` 决定是否递增版本号并放回节点。
pub(crate) struct SignalValueGuard {
    id: RawId,
    value: Option<AnyValue>,
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
