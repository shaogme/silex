//! 所有权与执行上下文：scope、销毁、清理函数、`untrack`、`batch`。

use crate::{AnyHandle, ScopeId, runtime::drive};

/// 建一个所有权 scope：`f` 里创建的节点都成为它的子节点，
/// [`dispose`] 这个 scope 会连带销毁它们（先子后父，同级按注册顺序）。
///
/// scope 本身**不是**计算节点：它里面的读取不建立任何依赖，它也不会重跑。
#[track_caller]
pub fn create<F: FnOnce()>(f: F) -> ScopeId {
    ScopeId::from_raw(drive::create_scope(f).expect("刚建出来的运行时可用"))
}

/// 建一个独立的 (detached) 所有权 scope：它的父节点是 `None`，不挂在当前 owner 下面。
/// 销毁外层 owner 时不会自动销毁它，调用者必须手动保存返回的 [`ScopeId`] 并由其掌控生命周期。
#[track_caller]
pub fn create_detached<R, F: FnOnce() -> R>(f: F) -> (ScopeId, R) {
    let (id, res) = drive::create_detached_scope(f).expect("刚建出来的运行时可用");
    (ScopeId::from_raw(id), res)
}


/// 销毁一个节点：跑它的清理函数、递归销毁子节点、退订它的全部依赖、
/// 释放它占用的存储。已经销毁过的句柄再传进来是 no-op。
///
/// 接受任何种类的句柄 —— 销毁对所有节点是同一件事。
pub fn dispose(id: impl AnyHandle) {
    drive::dispose(id.to_raw());
}

/// 注册一个清理函数，在当前节点被销毁或（对 effect 而言）下次重跑之前执行。
///
/// 当前没有正在运行的节点时什么都不做。
pub fn on_cleanup(f: impl FnOnce() + 'static) {
    drive::on_cleanup(f);
}

/// 在 `f` 执行期间关闭依赖追踪：里面读到的 signal 不会成为当前节点的依赖。
///
/// **只关追踪**。所有权上下文原封不动：`f` 里创建的节点照旧挂在当前 owner
/// 下面，随它一起销毁（AUDIT 二轮 §1.1）。
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    drive::untrack(f)
}

/// 把 `f` 里的所有写入合成一次调度：effect 队列直到最外层 `batch` 结束才执行。
///
/// 嵌套是允许的，只有最外层那次结束时才 flush。`f` panic 时深度由守卫恢复，
/// 不会把后续所有更新永久挂起（AUDIT P2）。
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    drive::batch(f)
}
