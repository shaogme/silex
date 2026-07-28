//! 响应式系统的外部驱动层。
//!
//! # 架构理念
//!
//! 驱动层负责桥接响应式 Runtime 存储与用户闭包的执行。
//! 访问 Runtime 的唯一途径是通过 [`with_rt`] 获取短期的 `&mut Runtime` 独占借用。
//! 所有的用户代码（计算闭包、Cleanup、析构 `Drop`、相等性比较 `PartialEq` 等）
//! 必须完全在 Runtime 借用之外执行。
//!
//! 跨越用户代码的状态（如求值栈、借出的值与闭包）保留在驱动函数的调用栈帧或 RAII 守卫中。

use crate::{
    DependencyList, ReactiveError, ReactiveResult,
    internal::{
        arena::RawId,
        value::{AnyValue, Computation, EffectThunk, MemoThunk},
    },
    runtime::{
        graph::{NodeState, Step},
        guard::{
            ComputationGuard, Depth, DepthGuard, EvalStack, NodeRunGuard, ObserverGuard,
            OwnerGuard, PayloadGuard, QueueGuard, SignalValueGuard,
        },
        scheduler::MAX_QUEUE_ITERATIONS,
        storage::{CleanupList, Debris, NodeFlags, NodeLinks, NodeMeta},
        with_rt, with_rt_or_init,
    },
};
use std::panic::Location;

// --- 节点读取准备 ---

/// 在读取节点之前完成必要的求值并建立依赖跟踪关系。
///
/// 遵循“先求值算干净，再建立依赖边”的顺序，确保登记的版本号为最新的求值结果。
#[inline]
pub(crate) fn prepare_read(id: RawId) {
    let settled = with_rt(|rt| {
        let settled = rt.is_settled(id);
        if settled {
            rt.track_dependency(id);
        }
        settled
    });
    if matches!(settled, Ok(true)) {
        return;
    }
    update_if_necessary(id);
    let _ = with_rt(|rt| rt.track_dependency(id));
}

/// 在无依赖追踪的情况下确保节点更新算干净。
#[inline]
pub(crate) fn prepare_read_untracked(id: RawId) {
    update_if_necessary(id);
}

/// 必要时沿依赖上游推演求值，将指定节点更新至 `Clean` 状态。
pub(crate) fn update_if_necessary(id: RawId) {
    if matches!(with_rt(|rt| rt.is_settled(id)), Ok(true)) {
        return;
    }

    let was_outermost = {
        let eval_guard = DepthGuard::enter(Depth::Evaluating);
        drive_eval(id);
        eval_guard.is_outermost()
    };

    if was_outermost {
        flush_if_idle();
    }
}

/// 增量求值的驱动循环：沿依赖拓扑关系向上将目标节点求值算干净。
///
/// # Panics
///
/// 当依赖成环或单次求值迭代超过 [`MAX_QUEUE_ITERATIONS`] 次时抛出 panic。
pub(crate) fn drive_eval(target_node: RawId) {
    let held = with_rt(|rt| {
        if rt.storage.get_state(target_node) == NodeState::Clean {
            return None;
        }
        let mut held = EvalStack::acquire(rt);
        held.get()
            .push(crate::runtime::graph::EvalFrame::new(target_node));
        Some(held)
    });
    let Ok(Some(mut held)) = held else { return };

    let mut iterations = 0usize;
    while let Ok(Step::Run(id)) = with_rt(|rt| rt.eval_step(held.get())) {
        iterations += 1;
        if iterations > MAX_QUEUE_ITERATIONS {
            let what = with_rt(|rt| rt.storage.describe(id)).unwrap_or_default();
            panic!(
                "silex_reactivity: 单次求值执行了超过 {MAX_QUEUE_ITERATIONS} 次计算仍未收敛，\
                 大概率是某个节点在自己的运行过程中写回了自己的上游。\
                 最后一个被求值的是 {what}。"
            );
        }

        if !run_node(id) {
            let _ = with_rt(|rt| rt.storage.set_state(id, NodeState::Clean));
        }

        held.get().pop();
    }
}

// --- 计算节点运行 ---

/// 运行计算节点（Effect 或 Memo）的计算闭包。
///
/// 在 Runtime 借用之外安全调用用户代码。
/// 若节点不存在、非计算节点或正在运行中，则返回 `false`。
pub(crate) fn run_node(id: RawId) -> bool {
    let Ok(Some(ticket)) = with_rt(|rt| rt.begin_run(id)) else {
        return false;
    };

    let run_guard = NodeRunGuard::new(id, ticket.computation);

    run_cleanups(id, ticket.children, ticket.cleanups, ticket.dependencies);

    let Some(computation) = run_guard.computation.as_ref() else {
        return false;
    };

    let is_memo = matches!(computation, Computation::Memo(_));
    let prepared = with_rt(|rt| {
        rt.storage.set_state(id, NodeState::Clean);
        let ctx = ComputationGuard::enter(rt, id);
        let old = is_memo.then(|| rt.take_signal_value(id).ok()).flatten();
        (ctx, old)
    });
    let Ok((mut ctx, old)) = prepared else {
        return false;
    };

    match computation {
        Computation::Effect(f) => f.call(),
        Computation::Memo(f) => recompute_memo(id, f, old),
    }

    let mut run_guard = run_guard;
    let _ = with_rt(|rt| {
        ctx.release(rt);
        run_guard.release(rt);
    });
    true
}

/// 重新计算 Memo 节点：借出旧值 → 执行计算闭包 → 比对变更 → 提交新值并通知下游。
fn recompute_memo(id: RawId, thunk: &MemoThunk, old: Option<AnyValue>) {
    let mut borrowed = old.map(|value| SignalValueGuard::new(id, value));

    let new_any = thunk.compute(borrowed.as_ref().and_then(SignalValueGuard::value));
    let changed = match borrowed.as_ref().and_then(SignalValueGuard::value) {
        Some(old) => !new_any.try_eq(old),
        None => true,
    };

    let mut new_any = Some(new_any);
    let (should_flush, buried) = with_rt(|rt| {
        if let Some(guard) = borrowed.as_mut() {
            guard.release(rt);
        }
        if !changed {
            return (false, false);
        }
        if rt.storage.meta(id).is_none() {
            return (false, false);
        }
        rt.storage
            .meta_mut(id)
            .expect("节点确认存在")
            .bump_version();
        let old = rt
            .storage
            .value_mut(id)
            .and_then(|slot| slot.replace(new_any.take().expect("单次重算仅提交一次")));
        let buried = old.is_some();
        if let Some(old) = old {
            rt.storage.bury(Debris::Payload(old));
        }
        rt.queue_dependents(id);
        (rt.should_flush(), buried)
    })
    .unwrap_or((false, false));
    drop(new_any);

    if buried {
        drain_graveyard();
    }
    if should_flush {
        run_queue();
    }
}

// --- 状态写入与调度 ---

/// 触发指定节点的变更通知并根据调度状态刷副作用队列。
pub(crate) fn notify_update(id: RawId) {
    let should_flush = with_rt(|rt| {
        rt.queue_dependents(id);
        rt.should_flush()
    });
    if matches!(should_flush, Ok(true)) {
        run_queue();
    }
}

/// 当系统空闲（无 batch 且无求值 DFS 进行中）时刷新副作用队列。
#[inline]
pub(crate) fn flush_if_idle() {
    if matches!(with_rt(|rt| rt.should_flush()), Ok(true)) {
        run_queue();
    }
}

/// 循环执行并清空副作用队列。
///
/// # Panics
///
/// 迭代次数超过 [`MAX_QUEUE_ITERATIONS`] 时抛出 panic。
pub(crate) fn run_queue() {
    let Some(_queue_guard) = QueueGuard::acquire() else {
        return;
    };

    let mut iterations = 0usize;
    while let Ok(Some(id)) = with_rt(|rt| rt.scheduler.observer_queue.pop_front()) {
        iterations += 1;
        if iterations > MAX_QUEUE_ITERATIONS {
            let what = with_rt(|rt| rt.storage.describe(id)).unwrap_or_default();
            panic!(
                "silex_reactivity: effect 队列执行超过 {MAX_QUEUE_ITERATIONS} 次仍未清空，\
                 大概率是若干 effect 在互相触发对方的依赖。最后一个被调度的是 {what}。"
            );
        }

        let runnable = with_rt(|rt| {
            rt.scheduler.queued_observers.remove(id);
            rt.storage.meta(id).is_some_and(NodeMeta::is_computation)
        });
        if matches!(runnable, Ok(true)) {
            update_if_necessary(id);
        }
    }
}

/// 可变修改 Signal 的值（通过 temporary-take 的方式交由闭包修改）。
///
/// 执行闭包期间值被暂存入 RAII 守卫，不允许在修改闭包内重入访问同一个 Signal。
pub(crate) fn with_signal_value_mut<R>(
    id: RawId,
    f: impl FnOnce(&mut AnyValue) -> (R, bool),
) -> ReactiveResult<R> {
    let taken = take_for_update(id)?;
    let mut borrowed = SignalValueGuard::new(id, taken);
    let (result, changed) = f(borrowed.value_mut());
    if changed {
        borrowed.bump_version_on_release();
    }
    Ok(result)
}

/// 只读访问 Signal 的值。闭包执行期间值暂存入守卫以保障重入安全。
pub(crate) fn with_signal_value<R>(id: RawId, f: impl FnOnce(&AnyValue) -> R) -> ReactiveResult<R> {
    let taken = with_rt(|rt| rt.take_signal_value(id))??;
    let borrowed = SignalValueGuard::new(id, taken);
    Ok(f(borrowed.value().expect("just moved in")))
}

/// 无类型擦除写入 Signal，仅当值发生实际改变时递增版本号并失效下游。
#[inline(never)]
pub(crate) fn update_signal_untyped(
    id: RawId,
    updater: &mut dyn FnMut(&mut AnyValue) -> bool,
) -> ReactiveResult<bool> {
    let taken = take_for_update(id)?;
    let mut borrowed = SignalValueGuard::new(id, taken);

    let applied = updater(borrowed.value_mut());
    if applied {
        borrowed.bump_version_on_release();
    }

    let should_flush = with_rt(|rt| {
        borrowed.release(rt);
        applied && {
            rt.queue_dependents(id);
            rt.should_flush()
        }
    })?;

    if should_flush {
        run_queue();
    }
    Ok(applied)
}

fn take_for_update(id: RawId) -> ReactiveResult<AnyValue> {
    match with_rt(|rt| rt.take_signal_value(id))? {
        Ok(value) => Ok(value),
        Err(e) => {
            debug_assert!(
                e != ReactiveError::Reentrant,
                "在 update 闭包内重入访问同一个 signal 是不被支持的"
            );
            Err(e)
        }
    }
}

// --- 非响应式载荷访问 ---

/// 访问非响应式载荷（Stored Value / Callback / NodeRef）。
pub(crate) fn with_payload<R>(id: RawId, f: impl FnOnce(&AnyValue) -> R) -> ReactiveResult<R> {
    let borrowed = with_rt(|rt| PayloadGuard::acquire(rt, id))??;
    Ok(f(borrowed.value()))
}

/// 可变访问非响应式载荷。
pub(crate) fn with_payload_mut<R>(
    id: RawId,
    f: impl FnOnce(&mut AnyValue) -> R,
) -> ReactiveResult<R> {
    let mut borrowed = with_rt(|rt| PayloadGuard::acquire(rt, id))??;
    Ok(f(borrowed.value_mut()))
}

// --- 创建节点 ---

#[track_caller]
pub(crate) fn create_signal(value: AnyValue) -> ReactiveResult<RawId> {
    let at = Location::caller();
    with_rt_or_init(|rt| rt.create_signal_at(at, value))
}

#[track_caller]
pub(crate) fn store_payload(value: AnyValue) -> ReactiveResult<RawId> {
    let at = Location::caller();
    with_rt_or_init(|rt| rt.store_payload_at(at, value))
}

#[track_caller]
pub(crate) fn create_effect(f: EffectThunk) -> ReactiveResult<RawId> {
    let at = Location::caller();
    let id = with_rt_or_init(|rt| {
        let id = rt.register_node_at(at);
        rt.storage.insert_reactive(
            id,
            NodeMeta::new(NodeState::Clean, NodeFlags::COMPUTATION),
            NodeLinks::default(),
            None,
            Some(crate::internal::value::Computation::Effect(f)),
        );
        id
    })?;

    let is_outermost = {
        let queue_guard = QueueGuard::acquire();
        let is_outermost = queue_guard.is_some();
        run_node(id);
        is_outermost
    };

    if is_outermost {
        flush_if_idle();
    }
    Ok(id)
}

#[track_caller]
pub(crate) fn create_memo(thunk: MemoThunk) -> ReactiveResult<RawId> {
    let at = Location::caller();
    let id = with_rt_or_init(|rt| {
        let id = rt.register_node_at(at);
        rt.prepare_memo_node(id, thunk);
        id
    })?;
    run_node(id);
    Ok(id)
}

// --- 所有权 Scope ---

/// 创建一个新的所有权作用域 Scope。
#[track_caller]
pub(crate) fn create_scope(f: impl FnOnce()) -> ReactiveResult<RawId> {
    let at = Location::caller();
    let (id, guards) = with_rt_or_init(|rt| {
        let id = rt.register_node_at(at);
        let owner = OwnerGuard::set(rt, Some(id));
        let observer = ObserverGuard::set(rt, None);
        (id, (owner, observer))
    })?;

    f();
    drop(guards);
    Ok(id)
}

/// 创建一个无父节点挂载的独立 (Detached) Scope。
#[track_caller]
pub(crate) fn create_detached_scope<R, F: FnOnce() -> R>(f: F) -> ReactiveResult<(RawId, R)> {
    let at = Location::caller();
    let (id, prev_owner_guard, inner_owner_guard, observer_guard) = with_rt_or_init(|rt| {
        let prev_owner = OwnerGuard::set(rt, None);
        let id = rt.register_node_at(at);
        let inner_owner = OwnerGuard::set(rt, Some(id));
        let observer = ObserverGuard::set(rt, None);
        (id, prev_owner, inner_owner, observer)
    })?;

    let res = f();
    drop(observer_guard);
    drop(inner_owner_guard);
    drop(prev_owner_guard);
    Ok((id, res))
}

/// 临时关闭依赖追踪执行 `f`（所有权层级不受影响）。
pub(crate) fn untrack<T>(f: impl FnOnce() -> T) -> T {
    let _observer = with_rt_or_init(|rt| ObserverGuard::set(rt, None)).ok();
    f()
}

/// 开启批量更新 `batch`：`f` 中的写操作延迟刷新 Effect 队列，直至最外层 `batch` 结束。
pub(crate) fn batch<R>(f: impl FnOnce() -> R) -> R {
    let _ = with_rt_or_init(|_| ());

    let result = {
        let _batch_guard = DepthGuard::enter(Depth::Batch);
        f()
    };

    flush_if_idle();
    result
}

pub(crate) fn on_cleanup(f: impl FnOnce() + 'static) {
    let thunk = crate::internal::value::OnceThunk::new(f);
    let _ = with_rt_or_init(|rt| rt.internal_on_cleanup(thunk));
}

// --- 节点销毁与清理 ---

/// 迭代式后序销毁步骤指示。
pub(crate) enum DisposeStep {
    /// 下降遍历：获取节点的子节点并入栈。
    Enter(RawId),
    /// 上升遍历：子树已销毁，执行节点自身的 Cleanup 回调与退订注销。
    Exit {
        id: RawId,
        cleanups: CleanupList,
        dependencies: DependencyList,
    },
}

/// 销毁指定节点及其底层结构与依赖订阅关系。
pub(crate) fn dispose_raw(id: RawId) {
    if matches!(
        with_rt(|rt| rt.storage.graph.get(id).is_none()),
        Ok(true) | Err(_)
    ) {
        return;
    }
    clean_node(id);

    let _ = with_rt(|rt| {
        let parent_id = rt.storage.graph.get(id).and_then(|n| n.parent);
        if let Some(parent_id) = parent_id
            && let Some(parent_aux) = rt.storage.node_aux.get_mut(parent_id)
            && let Some(idx) = parent_aux.children.iter().position(|&x| x == id)
        {
            parent_aux.children.swap_remove(idx);
        }
        rt.forget_node(id);
    });
    drain_graveyard();
}

/// 执行节点清理并摘除子节点/依赖，保留节点本身（用于 Effect 重新运行前）。
pub(crate) fn clean_node(id: RawId) {
    let taken = with_rt(|rt| {
        rt.storage.graph.get(id)?;
        let (children, cleanups) = rt.take_scope_state(id);
        Some((children, cleanups, rt.take_dependencies(id)))
    });
    let Ok(Some((children, cleanups, dependencies))) = taken else {
        return;
    };
    run_cleanups(id, children, cleanups, dependencies);
}

pub(crate) fn run_cleanups(
    self_id: RawId,
    children: Vec<RawId>,
    cleanups: CleanupList,
    dependencies: DependencyList,
) {
    dispose_subtrees(children);
    for cleanup in cleanups {
        cleanup.call();
    }
    if !dependencies.as_slice().is_empty() {
        let _ = with_rt(|rt| rt.unsubscribe(self_id, dependencies));
    }
}

/// 使用显式堆栈替代调用栈深度递归销毁节点子树，防止深层 Scope 节点树爆栈。
fn dispose_subtrees(roots: Vec<RawId>) {
    if roots.is_empty() {
        return;
    }

    let mut stack: Vec<DisposeStep> = Vec::with_capacity(roots.len());
    stack.extend(roots.into_iter().rev().map(DisposeStep::Enter));

    while let Some(step) = stack.pop() {
        match step {
            DisposeStep::Enter(id) => {
                let _ = with_rt(|rt| {
                    if rt.storage.graph.get(id).is_none() {
                        return;
                    }
                    let (children, cleanups) = rt.take_scope_state(id);
                    let dependencies = rt.take_dependencies(id);
                    stack.push(DisposeStep::Exit {
                        id,
                        cleanups,
                        dependencies,
                    });
                    stack.extend(children.into_iter().rev().map(DisposeStep::Enter));
                });
            }
            DisposeStep::Exit {
                id,
                cleanups,
                dependencies,
            } => {
                for cleanup in cleanups {
                    cleanup.call();
                }
                let _ = with_rt(|rt| {
                    rt.unsubscribe(id, dependencies);
                    rt.forget_node(id);
                });
                drain_graveyard();
            }
        }
    }
}

/// 排空并析构墓园中累积的残骸（在 Runtime 借用之外安全调用用户 `Drop`）。
pub(crate) fn drain_graveyard() {
    loop {
        let debris = with_rt(|rt| rt.storage.take_debris());
        let Ok(Some(debris)) = debris else { break };
        drop(debris);
    }
}

// --- 底层原始指针访问 ---

/// 获取节点中内部值的裸指针（用于无类型擦除直接只读访问）。
///
/// # Safety
///
/// 调用方必须确保数据类型一致，且在使用指针期间不进行任何使指针失效的操作。
pub(crate) unsafe fn get_any_raw_ptr_untracked(id: RawId) -> Option<*const ()> {
    prepare_read_untracked(id);

    with_rt(|rt| {
        unsafe {
            if let Some(value) = rt.signal_value_unchecked(id) {
                return Some(value.as_ptr());
            }
            rt.payload_value_unchecked(id).map(|v| v.as_ptr())
        }
    })
    .ok()
    .flatten()
}
