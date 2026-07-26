//! Silex 的响应式运行时：signal / memo / effect 三件套，以及它们背后的图算法。
//!
//! 这是一个**单线程**（每个线程一份运行时）、**基于句柄**的实现：所有节点都住在
//! 运行时的 arena 里，对外只交出一个 8 字节的 [`NodeId`]。上层框架（`silex_core`）
//! 在它之上包出带类型的 `Signal<T>` / `Memo<T>` 等门面。
//!
//! # 三类节点
//!
//! | 构造函数 | 是什么 | 何时重算 | 何时通知下游 |
//! |---|---|---|---|
//! | [`signal`] | 图的根，存一个值 | —— | **每一次成功的写入** |
//! | [`memo`] | 派生值，`T: Clone + PartialEq` | 惰性：被读时 | 仅当新值 `!=` 旧值 |
//! | [`register_derived`] | 派生值，`T: 'static` | 惰性：被读时 | **每一次重算** |
//! | [`effect`] | 副作用，无值 | 依赖变化后由队列调度 | —— |
//!
//! 相等性门控只有这一张表，别处不再有隐藏规则（AUDIT P10）。想要“值没变就别
//! 惊动下游”，要么用 [`memo`]，要么在写入侧用 [`set_signal_if_changed`]。
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
//! - **不要在 [`update_signal`] 的闭包里访问同一个 signal**：值在闭包执行期间被
//!   移出了节点。同理，不要在 [`memo`] 的计算闭包里读这个 memo 自己 —— 旧值请从
//!   闭包参数拿，它是借给你的，不是克隆给你的。
//! - **依赖成环会 panic**，报错里带上环上节点的调试标签与定义位置；effect 队列
//!   长时间不收敛（互相触发）同样 panic 而不是把线程挂死（AUDIT P13）。
//! - **同级 effect 的执行顺序不作承诺**：订阅者表用 swap-remove 维护，退订会打乱
//!   顺序（AUDIT P19.6）。有顺序要求请显式建立依赖关系。
//! - 用户代码 panic 之后运行时仍然可用：所有调度标志、借出的闭包与值都由 RAII
//!   守卫恢复（AUDIT P2）。
//! - 句柄可以随便复制和传递，但**它不保证节点还活着**。节点销毁后，读返回
//!   `None`，写是静默的 no-op。
//!
//! # `unsafe` 的边界
//!
//! 本 crate 内部大量使用类型擦除与裸指针（arena、`ThinVec`、`AnyValue`）。
//! 这些都是 crate 私有的：对外只有 [`NodeId`] 和几个显式标了 `unsafe` 的
//! 逃生出口（[`try_get_any_raw_untracked`]、[`try_get_stored_value_ref`]、
//! [`try_get_signal_value_ref`]），它们各自的 `# Safety` 段写明了什么操作会让
//! 返回的指针/引用失效。CI 里跑 `cargo miri test` 看着这条线。

mod core;
mod primitive;
mod runtime;

pub(crate) use crate::core::list::List;

/// 响应式节点的句柄。这是本 crate 唯一对外暴露的容器相关类型。
///
/// `Arena` / `SparseSecondaryMap` / `NodeState` 曾经也是 `pub` 的，但它们的
/// `get_mut(&self) -> Option<&mut T>` 允许安全代码两行就造出两个同时存活的
/// `&mut`（AUDIT P7）。用注释约束的契约必须由类型系统或 `unsafe` 表达，
/// 在此之前它们只能留在 crate 内部，由运行时自己保证独占访问。
pub use crate::core::arena::Index as NodeId;

use runtime::RUNTIME;
pub(crate) use runtime::Runtime;
use std::panic::Location;

pub use primitive::*;

pub(crate) type NodeList = List<NodeId>;
pub(crate) type DependencyList = List<(NodeId, u32)>;

/// 具有 16 字节对齐要求的 64 字节固定宽度缓冲区。
/// 用于跨 crate 安全地传递和存储类型擦除后的 Payload。
///
/// 缓冲区用 `MaybeUninit<u8>` 而不是 `u8`：Payload 里通常含有函数指针和数据指针，
/// 而整数类型的读写会擦除指针 provenance，按值搬运 `[u8; 64]` 之后再把这些字节
/// 当指针解引用即为未定义行为（AUDIT P3）。字节级复制则保留 provenance。
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct RawOpBuffer {
    data: [std::mem::MaybeUninit<u8>; 64],
}

impl Default for RawOpBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RawOpBuffer {
    pub const CAPACITY: usize = 64;
    pub const ALIGNMENT: usize = 16;

    /// 全零初始化的缓冲区。
    pub fn new() -> Self {
        Self {
            data: [std::mem::MaybeUninit::new(0); Self::CAPACITY],
        }
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr().cast()
    }

    #[inline(always)]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.data.as_mut_ptr().cast()
    }
}

/// 把 `f` 里的所有写入合成一次调度：effect 队列直到最外层 `batch` 结束才执行。
///
/// 嵌套是允许的，只有最外层那次结束时才 flush。`f` panic 时深度由守卫恢复，
/// 不会把后续所有更新永久挂起（AUDIT P2）。
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    RUNTIME.get_or(Runtime::new).batch(f)
}

/// 建一个所有权 scope：`f` 里创建的节点都成为它的子节点，
/// [`dispose`] 这个 scope 会连带销毁它们（先子后父，同级按注册顺序）。
#[track_caller]
pub fn create_scope<F>(f: F) -> NodeId
where
    F: FnOnce(),
{
    RUNTIME.get_or(Runtime::new).create_scope(f)
}

/// 销毁一个节点：跑它的清理函数、递归销毁子节点、退订它的全部依赖、
/// 释放它占用的存储。已经销毁过的句柄再传进来是 no-op。
pub fn dispose(id: NodeId) {
    if let Some(rt) = RUNTIME.get() {
        rt.dispose(id);
    }
}

/// 注册一个清理函数，在当前节点被销毁或（对 effect 而言）下次重跑之前执行。
///
/// 当前没有正在运行的节点时什么都不做。
pub fn on_cleanup(f: impl FnOnce() + 'static) {
    RUNTIME.get_or(Runtime::new).on_cleanup(f);
}

/// 在 `f` 执行期间关闭依赖追踪：里面读到的 signal 不会成为当前节点的依赖。
pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    RUNTIME.get_or(Runtime::new).untrack(f)
}

/// 获取任何响应式节点内部值的原始指针（signal 与 stored value 都行），
/// 供 `silex_core` 做去泛型化优化用。返回的指针指向 `T` 本身，不含任何类型信息。
///
/// # Safety
///
/// 调用方必须自己保证两件事：
///
/// 1. **类型对得上**：把它转成 `*const T` 时，`T` 必须就是当初存进去的类型。
///    本函数不做任何检查（这正是它比 `try_with_signal` 快的原因）。
/// 2. **指针还没失效**。下列操作**任意一条**都会让它悬垂，之后再解引用即为
///    未定义行为：
///    - [`dispose`] 该节点或它的任一祖先（arena 槽位被释放）；
///    - 写入该节点：[`update_signal`] / [`try_update_signal_silent`] 会把值移出
///      节点（期间里面是占位值），memo 重算后的提交会整体替换掉值，
///      [`try_update_stored_value`] 写入新值会 drop 掉旧值；
///    - 值从内联存储升级到堆上（`AnyValue` 的 SOO：小值直接放在节点里，
///      节点一旦被移动或替换，内联值的地址就变了）；
///    - **任何会重入运行时并执行用户代码的调用** —— effect 体、cleanup、
///      `batch` 收尾、乃至读一个 memo（会驱动惰性求值）—— 因为用户代码可以做
///      上面任意一件事。
///
/// 简而言之：拿到之后立刻用掉，不要跨越任何可能回到运行时的调用。
pub unsafe fn try_get_any_raw_untracked(id: NodeId) -> Option<*const ()> {
    let rt = RUNTIME.get()?;
    // SAFETY: 上面 `# Safety` 段里的两条契约（类型对得上、指针还没失效）
    // 原样转嫁给本函数的调用方，这里只是把节点里那个值的地址取出来。
    unsafe { rt.get_any_raw_ptr_untracked(id) }
}

/// 节点是在哪一行被创建的。
///
/// 只在 debug 构建下记录（release 恒为 `None`）。整条构造链路都带了
/// `#[track_caller]`，因此这里给出的是**用户的调用点**，不是框架内部
/// 某一行（AUDIT P11）。
pub fn get_node_defined_at(_id: NodeId) -> Option<&'static Location<'static>> {
    #[cfg(debug_assertions)]
    {
        let rt = RUNTIME.get()?;
        if let Some(node) = rt.storage.graph.get(_id) {
            return node.defined_at;
        }
        None
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
pub fn set_debug_label(_id: NodeId, _label: impl Into<String>) {
    #[cfg(debug_assertions)]
    {
        let label = _label.into();
        let rt = RUNTIME.get_or(Runtime::new);
        if let Some(aux) = rt.storage.try_aux_mut(_id) {
            aux.debug_label = Some(label);
        }
    }
}

/// 取回 [`set_debug_label`] 起的名字。
///
/// 节点销毁之后仍然能查到（运行时为最近若干个已销毁节点保留“墓碑”标签，
/// 数量有上限，见 AUDIT P14），这样“读一个已经销毁的节点”的报错才说得出
/// 它原来是谁。
pub fn get_debug_label(_id: NodeId) -> Option<String> {
    #[cfg(debug_assertions)]
    {
        let rt = RUNTIME.get()?;
        if let Some(aux) = rt.storage.node_aux.get(_id)
            && let Some(label) = &aux.debug_label
        {
            return Some(label.clone());
        }
        // Check dead labels
        rt.storage.dead_node_labels.get(_id).cloned()
    }
    #[cfg(not(debug_assertions))]
    {
        return None;
    }
}
