//! Silex 的响应式运行时：signal / memo / effect 三件套，以及它们背后的图算法。
//!
//! 这是一个**单线程**（每个线程一份运行时）、**基于句柄**的实现：所有节点都住在
//! 运行时的 arena 里，对外只交出一个 8 字节的带种类句柄（[`SignalId`]、
//! [`MemoId`]、[`StoredId`] …）。上层框架（`silex_core`）在它之上包出带类型的
//! `Signal<T>` / `Memo<T>` 等门面。
//!
//! # 三类节点
//!
//! | 构造函数 | 是什么 | 何时重算 | 何时通知下游 |
//! |---|---|---|---|
//! | [`signal::create`] | 图的根，存一个值 | —— | **每一次成功的写入** |
//! | [`memo::create`] | 派生值，`T: PartialEq` | 惰性：被读时 | 仅当新值 `!=` 旧值 |
//! | [`memo::derived`] | 派生值，`T: 'static` | 惰性：被读时 | **每一次重算** |
//! | [`effect::create`] | 副作用，无值 | 依赖变化后由队列调度 | —— |
//!
//! 相等性门控只有这一张表，别处不再有隐藏规则（AUDIT P10）。想要“值没变就别
//! 惊动下游”，要么用 [`memo::create`]，要么在写入侧用
//! [`signal::set_if_changed`]。
//!
//! # 模块地图
//!
//! | 模块 | 管什么 |
//! |---|---|
//! | [`signal`] | 创建 / 读 / 写 / 追踪 |
//! | [`memo`] | 两种派生节点 |
//! | [`effect`] | 副作用 |
//! | [`scope`] | 所有权、销毁、`on_cleanup`、`untrack`、`batch` |
//! | [`store`] | 非响应式的保管值 |
//! | [`callback`] | 类型擦除的回调 |
//! | [`node_ref`] | “稍后填充”的宿主元素引用 |
//!
//! 从前这里是 `pub use primitive::*` 把 40 个自由函数摊在 crate 根上，
//! 命名与错误语义各不相同（审计报告 §3.2）。现在按语义分模块，
//! 所有 `try_*` 一律返回 [`ReactiveResult`]。
//!
//! # 更新是怎么跑起来的
//!
//! 一次写入分两个阶段：
//!
//! 1. **传播**（BFS）：把下游标记为 `Dirty` / `Check`，把其中的 effect 推进队列；
//! 2. **求值**（DFS）：真正需要一个值的时候（读它，或轮到它的 effect 执行）才
//!    沿依赖向上求值，靠版本号判断“依赖到底变没变”，从而跳过不必要的重算。
//!
//! effect 只有一个调度出口：`batch` 结束、求值 DFS 结束、写入完成 —— 都归到同一个
//! “空闲时 flush 队列”的判断上，因此同一段用户代码的执行顺序不取决于它是首跑
//! 还是重跑（AUDIT P15）。菱形依赖不会看到中间态。
//!
//! # 调用方需要知道的几条契约
//!
//! - **不要在接受用户闭包的 API 里访问同一个节点**。[`signal::try_update`]、
//!   [`signal::try_with`]、[`store::try_with`]、[`store::try_update`]、
//!   memo 的计算闭包 —— 这些都会把值**移出**节点再交给闭包（节点里暂时是空的），
//!   这样运行时在用户代码执行期间不持有任何指向该节点的引用
//!   （AUDIT P5、审计报告 §2.1）。重入访问会拿到
//!   [`ReactiveError::Reentrant`]，而不是从前那种静默的别名违规。
//! - [`signal::try_get`] 会在运行时的独占借用内调用 `T::clone`。`clone` 里尝试
//!   重入运行时会得到 [`ReactiveError::Reentrant`]；不再存在 arena 引用需要用户
//!   手工维护的特殊例外。需要在读的时候运行任意用户代码请改用
//!   [`signal::try_with`]，那条路径会先把值移出节点。
//! - **依赖成环会 panic**，报错里带上环上节点的调试标签与定义位置；effect 队列
//!   长时间不收敛（互相触发）同样 panic 而不是把线程挂死（AUDIT P13）。
//! - **同级 effect 的执行顺序不作承诺**：订阅者表用 swap-remove 维护，退订会打乱
//!   顺序（AUDIT P19.6）。有顺序要求请显式建立依赖关系。
//! - 用户代码 panic 之后运行时仍然可用：所有调度标志、借出的闭包与值都由 RAII
//!   守卫恢复（AUDIT P2）。
//! - 句柄可以随便复制和传递，但**它不保证节点还活着**，用
//!   [`Handle::is_alive`] 查。
//!
//! # 种类安全
//!
//! 句柄带种类标记（见 `handle`）：`signal::try_get::<i32>(stored_id)` 是编译
//! 错误，不再是运行时的一个静默 `None`。需要跨种类传递时用 [`RawId`] 显式
//! 擦除 —— 那是唯一的逃生出口，也因此是唯一需要人工审查的地方。
//!
//! # `unsafe` 的边界
//!
//! 节点存储与 `Arena` / `SparseSecondaryMap` 是安全 Rust；arena 只提供带代数校验
//! 的独占写入口，旧句柄不会读到复用后的槽位。剩余的 `unsafe` 只集中在类型擦除
//! 的 `AnyValue`、紧凑的 `ThinVec` 和三个显式标记的
//! 原始引用逃生出口（[`try_get_any_raw_untracked`]、[`store::try_value_ref`]、
//! [`signal::try_value_ref`]）。这些 API 各自的 `# Safety` 段写明了返回指针/引用
//! 何时失效；CI 通过 `cargo +nightly miri test` 覆盖它们能运行到的路径。

// `mod runtime` / `mod internal` 都是私有的，里面的 `pub` 等价于 `pub(crate)` ——
// 读代码的人却会以为那是对外接口。开这条 lint 把两者区分开（审计报告 §3.4）。
#![deny(unreachable_pub)]

mod error;
mod handle;
// 从前叫 `mod core`，于是 crate 内部写 `core::mem::...` 会解析到它自己而不是
// 标准库（审计报告 §3.4）。
mod internal;
mod runtime;

pub mod callback;
pub mod effect;
pub mod memo;
pub mod node_ref;
pub mod scope;
pub mod signal;
pub mod store;

pub use crate::{
    error::{ReactiveError, ReactiveResult},
    handle::{
        AnyHandle, CallbackId, DerivedId, EffectId, Handle, MemoId, NodeKind, NodeRefId, RawId,
        Readable, ScopeId, SignalId, StoredId, kind,
    },
};

pub(crate) use crate::internal::list::List;
pub(crate) use runtime::Runtime;
use std::panic::Location;

pub(crate) type DependencyList = List<(RawId, u32, usize)>;

/// 获取一个已擦除种类的节点内部值的原始指针（signal 与 stored value 都行），
/// 供上层框架做去泛型化优化用。返回的指针指向 `T` 本身，不含任何类型信息。
///
/// # Safety
///
/// 调用方必须自己保证两件事：
///
/// 1. **类型对得上**：把它转成 `*const T` 时，`T` 必须就是当初存进去的类型。
///    本函数不做任何检查（这正是它比 [`signal::try_with`] 快的原因）。
/// 2. **指针还没失效**。下列操作**任意一条**都会让它悬垂，之后再解引用即为
///    未定义行为：
///    - [`scope::dispose`] 该节点或它的任一祖先（arena 槽位被释放）；
///    - 写入该节点：[`signal::try_update`] / [`signal::try_update_silent`] /
///      [`store::try_update`] 会把值移出节点（期间里面是空的），
///      memo 重算后的提交会整体替换掉值；
///    - 借用该节点：[`signal::try_with`] / [`store::try_with`] 同样会把值移出去；
///    - 值从内联存储升级到堆上（`AnyValue` 的 SOO：小值直接放在节点里，
///      节点一旦被移动或替换，内联值的地址就变了）；
///    - **任何会重入运行时并执行用户代码的调用** —— effect 体、cleanup、
///      `batch` 收尾、乃至读一个 memo（会驱动惰性求值）—— 因为用户代码可以做
///      上面任意一件事。
///
/// 简而言之：拿到之后立刻用掉，不要跨越任何可能回到运行时的调用。
pub unsafe fn try_get_any_raw_untracked(id: RawId) -> Option<*const ()> {
    // SAFETY: 上面 `# Safety` 段里的两条契约（类型对得上、指针还没失效）
    // 原样转嫁给本函数的调用方，这里只是把节点里那个值的地址取出来。
    unsafe { runtime::drive::get_any_raw_ptr_untracked(id) }
}

/// 节点是在哪一行被创建的。
///
/// 只在 debug 构建下记录（release 恒为 `None`）。整条构造链路都带了
/// `#[track_caller]`，因此这里给出的是**用户的调用点**，不是框架内部
/// 某一行（AUDIT P11）。
pub fn get_node_defined_at(_id: impl AnyHandle) -> Option<&'static Location<'static>> {
    #[cfg(debug_assertions)]
    {
        runtime::with_rt(|rt| rt.storage.graph.get(_id.into_raw())?.defined_at)
            .ok()
            .flatten()
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

// --- Debugging API ---

/// 给节点起一个便于排查问题的名字（只在 debug 构建下保存）。
///
/// 它会出现在依赖环、队列不收敛等诊断信息里。
pub fn set_debug_label(_id: impl AnyHandle, _label: impl Into<String>) {
    #[cfg(debug_assertions)]
    {
        let label = _label.into();
        let _ = runtime::with_rt_or_init(|rt| {
            rt.storage
                .with_aux_mut(_id.into_raw(), |aux| aux.debug_label = Some(label))
        });
    }
}

/// 取回 [`set_debug_label`] 起的名字。
///
/// 节点销毁之后仍然能查到（运行时为最近若干个已销毁节点保留“墓碑”标签，
/// 数量有上限，见 AUDIT P14），这样“读一个已经销毁的节点”的报错才说得出
/// 它原来是谁。
pub fn get_debug_label(_id: impl AnyHandle) -> Option<String> {
    #[cfg(debug_assertions)]
    {
        let raw = _id.into_raw();
        runtime::with_rt(|rt| {
            if let Some(aux) = rt.storage.node_aux.get(raw)
                && let Some(label) = aux.debug_label.clone()
            {
                return Some(label);
            }
            // Check dead labels
            rt.storage.dead_node_labels.get(raw).cloned()
        })
        .ok()
        .flatten()
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}
