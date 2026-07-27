//! 驱动层：运行时里**不持有任何借用**的那一半。
//!
//! # 这个模块存在的理由
//!
//! 访问运行时的唯一方式是 [`with_rt`]，它交出一个**独占**的 `&mut Runtime`，
//! 而这个借用出不了闭包。于是整个 crate 被一条线切成两半：
//!
//! | | 住在哪儿 | 能做什么 |
//! |---|---|---|
//! | **一次借用之内** | `Runtime` 上的方法 | 读写图、改状态、算下一步该干什么。**绝不执行一行用户代码** |
//! | **两次借用之间** | 本模块的自由函数 | 调用户的计算闭包、cleanup、`Drop`、`PartialEq`…… |
//!
//! 从前这条线只存在于注释里（“不要在持有节点引用时执行用户代码”），现在它
//! 由借用检查器画出来：想在借用之内调用户代码，得先把 `&mut Runtime` 走私出
//! 闭包 —— 编译不过。
//!
//! # 驱动循环长什么样
//!
//! 需要跨越用户代码的状态（求值 DFS 的栈、借出的计算闭包、借出的值）一律住在
//! **驱动帧上**，而不是运行时里：
//!
//! ```ignore
//! let mut held = with_rt(|rt| EvalStack::acquire(rt))?;   // 栈搬到本帧上
//! loop {
//!     let step = with_rt(|rt| rt.eval_step(held.get()))?; // 借用只活这一行
//!     let Step::Run(id) = step else { break };
//!     run_node(id);                                      // 用户代码，无借用
//! }
//! ```
//!
//! 因此**求值可以重入**：memo 的计算闭包里再读一个脏 memo，就是在 Rust 栈上
//! 再开一层驱动，各自持有自己的工作栈，互不干扰。同步惰性求值的语义原样保留 ——
//! 方案 B 当初被否掉的那条理由（“惰性语义要重新设计”）就是错在这里：它把
//! “命令队列”读成了“全局唯一的一条队列”。

use crate::{
    DependencyList, ReactiveError, ReactiveResult,
    internal::{
        arena::Index as NodeId,
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

// --- 读 ---

/// 读取一个节点之前：先把它算干净，**然后**才建立依赖边。
///
/// 顺序不能反过来。之前是先 `track_dependency` 再 `update_if_necessary`，
/// 于是当一个 memo 在自己的计算过程中第一次读到一个正处于 `Dirty` 的上游时：
///
/// 1. 它先把自己登记进上游的订阅者表；
/// 2. 上游随即重算、值变了、`commit_update` → `propagate` 把订阅者标脏 ——
///    而本节点刚刚才登记进去，且此刻**正在运行**；
/// 3. 本节点在运行前置的 `Clean` 被覆盖成 `Dirty`，跑完出栈时状态仍是 `Dirty`；
/// 4. 下游读它时看到 `Dirty`，再算一遍。
///
/// 实测每层恒定 2 倍（不随链长放大），用户的计算闭包被白跑一次。
/// 先求值再追踪之后，上游提交时本节点还不是它的订阅者，标不到自己头上；
/// 顺带把登记的版本号从“重算前”修正成“重算后”的正确值（AUDIT 二轮 §1.2）。
///
/// 代价：依赖边晚一步建立，因此依赖环要多绕一轮才会被求值检测到 ——
/// 仍然会被检测到，只是路径长一点。
#[inline]
pub(crate) fn prepare_read(id: NodeId) {
    // 快路径：节点已经干净、队列里也没有待办，求值与追踪在**同一次借用**里
    // 做完。绝大多数读取走的是这条（普通 signal 恒为干净）。
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

#[inline]
pub(crate) fn prepare_read_untracked(id: NodeId) {
    update_if_necessary(id);
}

/// 必要时把一个节点算干净。
///
/// 绝大多数读取的目标是一个**永远干净的普通 signal**，而整套求值机制对它们
/// 是纯粹的浪费：借出工作栈、看一眼状态就返回、再还回去，外加一个深度守卫和
/// 一次空队列的 `run_queue`。实测这条提前返回让无 owner 上下文的 signal 读取
/// 从 20.5 ns 降到 11.6 ns（−43%）（AUDIT 二轮 §1.3）。
pub(crate) fn update_if_necessary(id: NodeId) {
    if matches!(with_rt(|rt| rt.is_settled(id)), Ok(true)) {
        return;
    }

    let was_outermost = {
        // DFS 期间禁止 flush effect 队列（AUDIT P15）。
        let eval_guard = DepthGuard::enter(Depth::Evaluating);
        drive_eval(id);
        eval_guard.is_outermost()
    };

    // DFS 期间被推迟的更新在这里统一 flush。
    // 若本次求值本身就发生在队列执行中，`run_queue` 的守卫会让这次调用直接返回，
    // 由外层的队列循环继续消费。
    if was_outermost {
        flush_if_idle();
    }
}

/// 求值的驱动循环：沿依赖向上把一个节点算干净。
///
/// # Panics
///
/// 依赖成环时 panic（见 [`Runtime::eval_step`]）；单次求值执行的计算次数超过
/// [`MAX_QUEUE_ITERATIONS`] 时同样 panic —— AUDIT P13 当初只给 effect 队列设了
/// 上限，漏掉了这一半：一个节点只要在自己的运行过程中（计算闭包里、或者被覆盖
/// 的旧值的 `Drop` 里）写回自己的上游，就会被立刻重新标脏，DFS 于是原地重跑它，
/// 永不收敛 —— 而这个循环从头到尾没有碰过 effect 队列，`run_queue` 的计数器
/// 一次都不会加。表现就是浏览器标签页直接冻死，正是 P13 要消灭的那种失败。
pub(crate) fn drive_eval(target_node: NodeId) {
    let held = with_rt(|rt| {
        if rt.storage.get_state(target_node) == NodeState::Clean {
            return None;
        }
        // 工作栈搬到本帧上：它必须跨越 `run_node`（用户代码），
        // 因此绝不能是运行时里的一块状态。成环 panic 时由守卫归还池子。
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

        // 状态转换由 `run_node` 负责（运行前置 Clean）。这里**不能**再无条件
        // 写一次 Clean —— 那会把节点在自己运行期间产生的失效标记抹掉，
        // 使得队列里的重跑条目被当作“已干净”跳过，更新静默丢失（AUDIT P8）。
        if !run_node(id) {
            // 不是计算节点（例如一个被标脏的普通 signal）：必须置 Clean，
            // 否则上游会反复把它压栈。
            let _ = with_rt(|rt| rt.storage.set_state(id, NodeState::Clean));
        }

        // 无论本次是否重新变脏都要出栈：重新变脏意味着它已经被重新入队，
        // 由调度队列负责重跑；留在栈上只会死循环。
        held.get().pop();
    }
}

// --- 运行一个计算节点 ---

/// 运行一个计算节点（effect 或 memo）的计算闭包。
///
/// 这是**唯一**执行用户计算的入口 —— effect 首跑、effect 重跑、memo 首算、
/// memo 重算全部走这里。之前 `run_effect` 与 `run_computation` 是同一段逻辑的
/// 两份拷贝，各自演化出不同的状态转换，正是 P1 / P8 得以存在的土壤（AUDIT P16）。
///
/// 三段之间各借一次运行时，用户代码落在借用之外：
/// 取票（把闭包与上一次运行的残留整个移出节点）→ 跑 cleanup → 跑计算。
///
/// 返回值表示是否真的执行了计算闭包。以下情况返回 `false`：
/// 节点不存在、不是计算节点、或**正在运行中**。
pub(crate) fn run_node(id: NodeId) -> bool {
    let Ok(Some(ticket)) = with_rt(|rt| rt.begin_run(id)) else {
        return false;
    };

    // 从这里开始，闭包的归还与重入锁的释放由守卫接管（panic 展开时同样生效）。
    let run_guard = NodeRunGuard::new(id, ticket.computation);

    // 清理上一次运行留下的子节点、cleanup 与订阅关系。
    run_cleanups(id, ticket.children, ticket.cleanups, ticket.dependencies);

    let Some(computation) = run_guard.computation.as_ref() else {
        return false;
    };

    // 状态在调用用户闭包**之前**置 Clean。运行期间产生的失效标记
    // （例如 effect 写了自己的依赖）因此得以保留，不会被“运行完再无条件置
    // Clean”抹掉（AUDIT P8）。
    //
    // memo 的旧值也在这一次借用里一并借出：置状态、进上下文、取旧值三件事
    // 之间没有一行用户代码，分成两次借用纯粹是白付一次借用计数。
    let is_memo = matches!(computation, Computation::Memo(_));
    let prepared = with_rt(|rt| {
        rt.storage.set_state(id, NodeState::Clean);
        let ctx = ComputationGuard::enter(rt, id);
        // 首算时节点里本来就没有值；节点不存在、或值正被某个 update 闭包借出时
        // 同样没有可用的旧值 —— 一律按“变了”处理。
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

    // 收尾：退出计算上下文、归还闭包、解除重入锁 —— 合成一次借用。
    // （`computation` 对 `run_guard` 的借用到上面那个 `match` 为止。）
    let mut run_guard = run_guard;
    let _ = with_rt(|rt| {
        ctx.release(rt);
        run_guard.release(rt);
    });
    true
}

/// 重算一个 memo：借出旧值 → 调用计算闭包 → 与旧值比较 → 提交。
///
/// 旧值是**借**给计算闭包的，不是克隆给它的。之前这里为一次重算克隆旧值三次
/// （节点里克隆一份、再克隆一份传给 vtable、vtable 里再 `cloned()` 一次），
/// 而且闭包用不用 `old` 都要付这个代价 —— 对持有 `Vec` / `String` 的 memo
/// 就是每次重算三次深拷贝（AUDIT P9）。
///
/// 旧值在计算期间被**移出**节点，理由与写入相同：计算闭包是用户代码，运行时
/// 不能在它执行期间持有指向节点载荷的借用（AUDIT P5）。因此“在 memo 的计算
/// 闭包里读它自己”读到的是 [`ReactiveError::Reentrant`]，旧值只能从闭包参数拿
/// —— 这本来也是 `Fn(Option<&T>) -> T` 这个签名的用途。
fn recompute_memo(id: NodeId, thunk: &MemoThunk, old: Option<AnyValue>) {
    // 守卫保证旧值一定会被放回（计算闭包 panic 时也一样）。
    let mut borrowed = old.map(|value| SignalValueGuard::new(id, value));

    // --- 以下两步是用户代码，必须在借用之外 ---
    let new_any = thunk.compute(borrowed.as_ref().and_then(SignalValueGuard::value));
    // 比较也在旧值还被借出时进行：`try_eq` 会调用用户的 `PartialEq`。
    let changed = match borrowed.as_ref().and_then(SignalValueGuard::value) {
        Some(old) => !new_any.try_eq(old),
        None => true,
    };

    // 归还旧值 → 写入新值（被覆盖的旧值进墓园）→ 传播 → 判定要不要 flush。
    // 这四步之间不执行任何用户代码，因此合成**一次**借用 —— 拆开写的话一次
    // memo 重算要为此多付四次线程本地查表，而重算是这个 crate 最热的写路径。
    //
    // 被覆盖掉的旧值走墓园而不是就地析构：它装的是用户数据，析构就是执行
    // 用户的 `Drop`，而用户的 `Drop` 可以回头访问响应式图。
    let mut new_any = Some(new_any);
    let (should_flush, buried) = with_rt(|rt| {
        if let Some(guard) = borrowed.as_mut() {
            guard.release(rt);
        }
        if !changed {
            return (false, false);
        }
        // 节点已经没了：新值原样留在 `new_any` 里，在借用之外析构。
        if rt.storage.meta(id).is_none() {
            return (false, false);
        }
        rt.storage
            .meta_mut(id)
            .expect("节点刚刚确认存在")
            .bump_version();
        let old = rt
            .storage
            .value_mut(id)
            .and_then(|slot| slot.replace(new_any.take().expect("每次重算只提交一次")));
        let buried = old.is_some();
        if let Some(old) = old {
            rt.storage.bury(Debris::Payload(old));
        }
        rt.queue_dependents(id);
        (rt.should_flush(), buried)
    })
    .unwrap_or((false, false));
    drop(new_any);

    // 用户的 `Drop` 与下游 effect 都必须在借用之外。排空点在传播之后、
    // 队列执行之前 —— 与从前“写入之后、通知之前”的可观察顺序一致，因为
    // 传播自己不跑一行用户代码。
    if buried {
        drain_graveyard();
    }
    if should_flush {
        run_queue();
    }
}

// --- 写与调度 ---

pub(crate) fn notify_update(id: NodeId) {
    // 传播与“该不该 flush”的判定合成一次借用 —— 两者之间不执行任何用户代码。
    let should_flush = with_rt(|rt| {
        rt.queue_dependents(id);
        rt.should_flush()
    });
    if matches!(should_flush, Ok(true)) {
        run_queue();
    }
}

/// 在没有 batch、也没有正在进行的求值 DFS 时执行 effect 队列。
///
/// 这是 effect 的**唯一**调度出口：所有会产生失效的路径都汇聚到这里，
/// 执行时机不再取决于调用方走的是哪条入口（AUDIT P15）。
#[inline]
pub(crate) fn flush_if_idle() {
    if matches!(with_rt(|rt| rt.should_flush()), Ok(true)) {
        run_queue();
    }
}

/// 执行 effect 队列直到清空。
///
/// # Panics
///
/// 单次执行超过 [`MAX_QUEUE_ITERATIONS`] 次迭代时 panic。互相写入对方依赖的
/// 两个 effect 会让队列永远不空，之前既没有上限也没有诊断，表现就是浏览器
/// 标签页直接冻死（AUDIT P13）。
pub(crate) fn run_queue() {
    // 守卫保证标志一定会被恢复：裸写法在 effect panic 时会让 `running_queue`
    // 永久卡在 true，此后 `run_queue` 每次入口直接返回，整个响应式系统静默停摆
    // （AUDIT P2）。`acquire` 返回 None 表示外层已经在跑队列。
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

/// 以“取出 → 交给用户闭包 → 放回”的方式修改 signal 的值。
///
/// 用户闭包执行期间，节点里的值是 `None`，运行时不再持有任何指向该节点载荷
/// 的借用 —— 否则闭包内一旦重入访问同一个节点（哪怕只是读一下），就会构造出
/// 与之重叠的引用，这是实打实的 UB（AUDIT P5）。
///
/// 代价是一条明确的契约：**不允许在 update 闭包内访问同一个 signal**。
/// debug 构建下会断言失败，release 下该次访问返回 [`ReactiveError::Reentrant`]。
///
/// 版本号由 `f` 的第二个返回值决定：`true` 表示“值真的被改写了”，
/// 此时版本号在**归还值的那一次查表里**顺带递增（AUDIT P12 定下语义）。
pub(crate) fn with_signal_value_mut<R>(
    id: NodeId,
    f: impl FnOnce(&mut AnyValue) -> (R, bool),
) -> ReactiveResult<R> {
    let taken = take_for_update(id)?;

    // 守卫保证值一定会被放回（panic 展开时也一样）。
    let mut borrowed = SignalValueGuard::new(id, taken);
    let (result, changed) = f(borrowed.value_mut());
    if changed {
        borrowed.bump_version_on_release();
    }
    Ok(result)
}

/// 把 signal 的值移出节点、交给**用户闭包**、再放回去（只读版本）。
///
/// 只读也要移出：闭包是用户代码，它可以销毁任何节点 —— 包括这一个。
/// 代价是与写入侧一致的一条契约：**不允许在闭包内访问同一个 signal**，
/// 否则拿到 [`ReactiveError::Reentrant`]。
pub(crate) fn with_signal_value<R>(
    id: NodeId,
    f: impl FnOnce(&AnyValue) -> R,
) -> ReactiveResult<R> {
    let taken = with_rt(|rt| rt.take_signal_value(id))??;
    // 守卫保证值一定会被放回（闭包 panic 时也一样），且不递增版本号。
    let borrowed = SignalValueGuard::new(id, taken);
    Ok(f(borrowed.value().expect("just moved in")))
}

/// 写入 signal 并在写入真的发生时失效下游。
///
/// `updater` 返回 `false` 表示它没有改动这个值（典型情况是类型不匹配）：
/// 此时既不递增版本号也不通知下游 —— 之前的写法无条件递增并通知，
/// 于是一次什么都没做的更新会静默地把全部下游重跑一遍（AUDIT P12）。
#[inline(never)]
pub(crate) fn update_signal_untyped(
    id: NodeId,
    updater: &mut dyn FnMut(&mut AnyValue) -> bool,
) -> ReactiveResult<bool> {
    let taken = take_for_update(id)?;
    let mut borrowed = SignalValueGuard::new(id, taken);

    // 用户代码。此刻节点里的值是 `None`，运行时也没有任何借用（AUDIT P5）。
    let applied = updater(borrowed.value_mut());
    if applied {
        // “请递增版本号”与“写入真的发生了”是同一件事（AUDIT P12）。
        borrowed.bump_version_on_release();
    }

    // 归还值、传播、判定要不要 flush —— 三件事之间不执行任何用户代码，
    // 因此合成一次借用。分开写的话 0 订阅者的写入要付 4 次线程本地查表。
    let should_flush = with_rt(|rt| {
        borrowed.release(rt);
        applied && {
            rt.queue_dependents(id);
            rt.should_flush()
        }
    })?;

    // 队列执行必须在借用之外：它会同步跑下游 effect，那些 effect 会重新
    // 访问本节点。
    if should_flush {
        run_queue();
    }
    Ok(applied)
}

/// 把值移出节点，并在“重入了同一个 signal”这条编程错误上给出断言。
fn take_for_update(id: NodeId) -> ReactiveResult<AnyValue> {
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

// --- 非响应式载荷 ---

/// 把载荷移出节点、交给**用户闭包**、再放回去。
///
/// 这是所有会执行用户代码的载荷访问的唯一入口（审计报告 §2.1）。
pub(crate) fn with_payload<R>(id: NodeId, f: impl FnOnce(&AnyValue) -> R) -> ReactiveResult<R> {
    let borrowed = with_rt(|rt| PayloadGuard::acquire(rt, id))??;
    Ok(f(borrowed.value()))
}

/// 同上，可变版本。
pub(crate) fn with_payload_mut<R>(
    id: NodeId,
    f: impl FnOnce(&mut AnyValue) -> R,
) -> ReactiveResult<R> {
    let mut borrowed = with_rt(|rt| PayloadGuard::acquire(rt, id))??;
    Ok(f(borrowed.value_mut()))
}

// --- 建节点 ---

#[track_caller]
pub(crate) fn create_signal(value: AnyValue) -> ReactiveResult<NodeId> {
    let at = Location::caller();
    with_rt_or_init(|rt| rt.create_signal_at(at, value))
}

#[track_caller]
pub(crate) fn store_payload(value: AnyValue) -> ReactiveResult<NodeId> {
    let at = Location::caller();
    with_rt_or_init(|rt| rt.store_payload_at(at, value))
}

#[track_caller]
pub(crate) fn create_effect(f: EffectThunk) -> ReactiveResult<NodeId> {
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

    // 首跑必须与重跑走同一条调度路径：先占住 `running_queue`，
    // 让 effect 体内的写入只入队、不在体内嵌套 flush，运行结束后统一 flush。
    // 否则同一段用户代码的执行顺序会取决于它是首跑还是重跑（AUDIT P15），
    // 而嵌套 flush 更会重入到这个正在运行的 effect 上（AUDIT P1）。
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

/// 建一个 memo / derived 节点，装上计算闭包并立即完成首次计算。
///
/// 闭包先被装进节点，再由统一的 [`run_node`] 驱动首跑：这样首跑与后续重算走
/// 同一条路径，也不存在“闭包尚未被节点接管就提前返回”导致析构函数永不运行的
/// 窗口（AUDIT P19.10）。
#[track_caller]
pub(crate) fn create_memo(thunk: MemoThunk) -> ReactiveResult<NodeId> {
    let at = Location::caller();
    let id = with_rt_or_init(|rt| {
        let id = rt.register_node_at(at);
        rt.prepare_memo_node(id, thunk);
        id
    })?;
    run_node(id);
    Ok(id)
}

// --- 所有权上下文 ---

/// 建一个所有权 scope。
///
/// scope 只是一个所有权容器，它自己不是计算节点，没有“重跑”这回事，
/// 因此不能成为 observer —— 里面的读取一律不建立依赖。
#[track_caller]
pub(crate) fn create_scope(f: impl FnOnce()) -> ReactiveResult<NodeId> {
    let at = Location::caller();
    let (id, guards) = with_rt_or_init(|rt| {
        let id = rt.register_node_at(at);
        let owner = OwnerGuard::set(rt, Some(id));
        let observer = ObserverGuard::set(rt, None);
        (id, (owner, observer))
    })?;

    // 借用已经释放，用户代码在这里跑。
    f();
    drop(guards);
    Ok(id)
}

/// 在 `f` 执行期间关闭依赖追踪 —— **只关追踪**。
///
/// 所有权上下文原封不动：`f` 里创建的节点照旧挂在当前 owner 下面，随它一起
/// 销毁。之前这里连 owner 一起清掉，于是 `untrack(|| store_value(v))` 这种
/// “只是想避免建立依赖” 的写法会造出一个永远回收不掉的孤儿节点，而
/// `silex_core` 的每一个 `Rx::new_op` / `new_constant` 都是这么写的
/// （AUDIT 二轮 §1.1）。
pub(crate) fn untrack<T>(f: impl FnOnce() -> T) -> T {
    // 守卫保证 observer 一定会被恢复：裸写法在 f panic 时会让追踪永久关闭
    // （AUDIT P2）。
    let _observer = with_rt_or_init(|rt| ObserverGuard::set(rt, None)).ok();
    f()
}

/// 把 `f` 里的所有写入合成一次调度：effect 队列直到最外层 `batch` 结束才执行。
pub(crate) fn batch<R>(f: impl FnOnce() -> R) -> R {
    // batch 是一个会**建**运行时的入口（用户可能在里面创建第一个节点）。
    let _ = with_rt_or_init(|_| ());

    // 守卫保证深度一定会被恢复：裸写法在 f panic 时会让 `batch_depth` 卡在非零，
    // 此后所有更新被永久挂起（AUDIT P2）。
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

// --- 销毁 ---

/// 显式销毁工作栈的一帧（AUDIT P19.8）。
///
/// 销毁本质上是一次后序遍历：先递归销毁子树，再跑自己的 cleanup。原来的实现
/// 直接用调用栈来表达这个“后序”，递归深度等于组件树深度，深层树会栈溢出。
/// 这里把调用栈搬到堆上：`Enter` 负责下降（摘下 children 并把它们排进工作栈），
/// `Exit` 负责上升（跑 cleanup、退订、抹除节点）。
pub(crate) enum DisposeStep {
    /// 下降：摘下节点的 children / cleanups / dependencies，把子树排进工作栈。
    Enter(NodeId),
    /// 上升：子树已经全部销毁，轮到节点自己。
    ///
    /// cleanups 与 dependencies 在 `Enter` 阶段就已经摘下来随帧带走 —— 这一点很
    /// 关键：cleanup 闭包可能反过来销毁本节点，届时它读到的是一份空列表，
    /// 不会把同一批 cleanup 跑第二遍。
    Exit {
        id: NodeId,
        cleanups: CleanupList,
        dependencies: DependencyList,
    },
}

/// 销毁一个节点：跑它的清理函数、递归销毁子节点、退订它的全部依赖、
/// 释放它占用的存储。已经销毁过的句柄再传进来是 no-op。
pub(crate) fn dispose(id: NodeId) {
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

/// 跑一个节点的清理，但**保留节点本身**（effect 重跑之前走这条）。
pub(crate) fn clean_node(id: NodeId) {
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

/// 顺序与原先的递归实现完全一致：子树先于自身，同级按注册顺序，
/// 自身的 cleanup 跑完之后才解除订阅。
pub(crate) fn run_cleanups(
    self_id: NodeId,
    children: Vec<NodeId>,
    cleanups: CleanupList,
    dependencies: DependencyList,
) {
    dispose_subtrees(children);
    for cleanup in cleanups {
        cleanup.call();
    }
    // 没有依赖就不必为退订取一次借用（effect 首跑、纯 scope 都是这样）。
    if !dependencies.as_slice().is_empty() {
        let _ = with_rt(|rt| rt.unsubscribe(self_id, dependencies));
    }
}

/// 用显式工作栈销毁若干棵子树（含根），栈深度不再受组件树深度限制（AUDIT P19.8）。
///
/// 遍历顺序严格等价于原来的递归：对每个节点，先按注册顺序逐棵销毁子树，
/// 再跑自己的 cleanup、退订、抹除自身。注意“摘下 children”这一步也是**惰性**的
/// —— 只有轮到某个节点被 `Enter` 时才去读它的 children，因此前一个兄弟的 cleanup
/// 闭包对后一个兄弟做的任何改动都仍然可见，和递归时一模一样。
fn dispose_subtrees(roots: Vec<NodeId>) {
    if roots.is_empty() {
        // 绝大多数节点没有子节点（effect 每次重跑都会走到这里），什么都不做。
        return;
    }

    // 工作栈住在本帧上：它要跨越 cleanup（用户代码）。
    let mut stack: Vec<DisposeStep> = Vec::with_capacity(roots.len());
    // 逆序压栈，弹出时才是注册顺序。
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
                    // 子节点不需要从父节点的 children 里摘除：父节点的那份列表
                    // 早在 `Enter` 阶段就被整体 take 走了。
                    rt.forget_node(id);
                });
                // 载荷的析构（用户的 `Drop`）就在这里，与从前就地析构的时机
                // 相同：本节点处理完、下一个节点开始之前。
                drain_graveyard();
            }
        }
    }
}

/// 析构墓园里的一切 —— 这里跑的是用户的 `Drop`，因此必须在借用之外。
pub(crate) fn drain_graveyard() {
    loop {
        let debris = with_rt(|rt| rt.storage.take_debris());
        let Ok(Some(debris)) = debris else { break };
        drop(debris);
    }
}

// --- 逃生出口 ---

/// 取出节点内部值的裸指针（signal 与 stored value 都支持）。
///
/// # Safety
///
/// 契约见 [`crate::try_get_any_raw_untracked`]：调用方负责类型正确，
/// 并保证在使用期间不发生任何会让该地址失效的操作。
pub(crate) unsafe fn get_any_raw_ptr_untracked(id: NodeId) -> Option<*const ()> {
    // 先把节点算干净，**再**取指针。少了这一步，读一个脏 memo / derived 会
    // 安静地拿到上一轮的值 —— 上层框架的类型擦除读取路径正是走这里，
    // 而它读到的可能是任何一种可读节点（AUDIT 二轮 §1.3）。
    //
    // 顺序不能反：求值会执行用户代码（memo 闭包、被冲掉的 effect 队列），
    // 那正是本函数的 `# Safety` 段里说的“会让指针失效”的操作。
    prepare_read_untracked(id);

    with_rt(|rt| {
        // SAFETY: 契约转嫁给调用方（见本函数的 `# Safety`）。
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
