//! 运行时状态的 RAII 守卫。
//!
//! 运行时的每一个“设标志 → 调用用户代码 → 恢复标志”的位置都必须用守卫来恢复。
//! 裸写法在用户代码 panic 时会让标志永远卡住：`running_queue` 卡在 `true` 会让
//! 整个响应式系统静默停摆，`batch_depth` 卡在非零会让所有更新被永久挂起，
//! 被 `take()` 出来的计算闭包则会永久丢失（AUDIT P2）。

use crate::{
    core::{arena::Index as NodeId, value::AnyValue, value::ThunkValue},
    runtime::Runtime,
};
use std::cell::Cell;

/// 恢复 `current_owner`。
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

/// 节点运行期间“借出”计算闭包。
///
/// 闭包必须先从节点里取出来才能在不持有 `&mut ReactiveNode` 的情况下调用，
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
        if let Some(node) = self.rt.storage.reactive.get_mut(self.id)
            && let Some(effect) = node.effect.as_mut()
        {
            effect.running = false;
            if let Some(f) = self.computation.take() {
                effect.computation = Some(f);
            }
        }
        // 节点在自己的运行期间被销毁时走到这里：`computation` 随守卫一起析构。
    }
}

/// signal 值在用户闭包执行期间的“借出”状态。
///
/// 值被移出节点、放在守卫里交给用户闭包，节点内暂时是一个占位值。
/// 这样运行时在用户代码执行期间不持有任何指向该节点的 `&mut`（AUDIT P5）。
pub(crate) struct SignalValueGuard<'a> {
    rt: &'a Runtime,
    id: NodeId,
    value: Option<AnyValue>,
}

impl<'a> SignalValueGuard<'a> {
    pub(crate) fn new(rt: &'a Runtime, id: NodeId, value: AnyValue) -> Self {
        Self {
            rt,
            id,
            value: Some(value),
        }
    }

    pub(crate) fn value_mut(&mut self) -> &mut AnyValue {
        self.value.as_mut().expect("signal value is borrowed out")
    }
}

impl Drop for SignalValueGuard<'_> {
    fn drop(&mut self) {
        if let Some(node) = self.rt.storage.reactive.get_mut(self.id)
            && let Some(signal) = node.signal.as_mut()
        {
            signal.updating = false;
            if let Some(value) = self.value.take() {
                signal.value = value;
            }
        }
        // 节点在闭包执行期间被销毁时走到这里：值随守卫一起析构。
    }
}
