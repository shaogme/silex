//! 副作用节点。

use crate::{EffectId, internal::value::EffectThunk, runtime::drive};

/// 创建一个 effect：立即运行一次 `f`，之后每当它读过的任一 signal 变化就重跑。
///
/// - 依赖是**动态**的：每次运行都会重新收集，上一轮读过、这一轮没读的 signal
///   会被自动退订。
/// - 重跑之前会执行本次运行内 [`on_cleanup`](crate::scope::on_cleanup) 注册的
///   清理函数，并销毁本次运行创建的子节点。
/// - 在 effect 体内写 signal 是允许的：写入只会入队，等本次运行结束后再统一
///   调度，首次运行与后续重跑的时序完全一致（AUDIT P1 / P15）。
/// - 若干 effect 互相触发对方的依赖会让队列永远不空，运行时会在若干次迭代后
///   panic 并报出最后调度的节点，而不是把线程挂死（AUDIT P13）。
///
/// # `FnMut`
///
/// `f` 只需要是 `FnMut`：想在 effect 里维护一点状态（计数器、上一次的值）
/// 直接 `move` 进去就行，不必自己套一层 `Cell` / `RefCell`（审计报告 §3.4）。
/// 这在本模型下是安全的 —— 同一个节点在同一时刻只可能有一次执行，`run_node`
/// 的 `running` 标志会让重入的那次直接返回（AUDIT P1）。
///
/// 已有的 `Fn` 闭包不受影响（`Fn: FnMut`），但**类型推断会变**：
/// 传一个会捕获可变状态的闭包现在能编译过了，从前编译不过。
#[track_caller]
pub fn create<F: FnMut() + 'static>(f: F) -> EffectId {
    EffectId::from_raw(drive::create_effect(EffectThunk::new(f)).expect("刚建出来的运行时可用"))
}
