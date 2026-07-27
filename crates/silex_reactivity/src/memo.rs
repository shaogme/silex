//! 惰性求值的派生节点：[`create`]（带相等性门控）与 [`derived`]（不带）。
//!
//! 两者的读取都走 [`signal`](crate::signal) 模块 —— [`MemoId`] 与 [`DerivedId`]
//! 都实现了 [`Readable`](crate::Readable)。
//!
//! # 这里为什么不再有手写 vtable
//!
//! 这个模块从前有三张手写的 `MemoVTable`（内联 / 装箱 / derived）加一个
//! `build_memo_payload`：把 vtable 指针写进 `InlineStorage` 的偏移 0、闭包写在
//! 其后，再由 `runtime.rs` 里的一个“通用 runner”在运行时读回来分派。
//!
//! 那一层间接存在的唯一理由是“`run_node` 只想认识一种 thunk 类型”——
//! effect 是 `FnMut()`、memo 是 `Fn(Option<&T>) -> T`，签名对不上，就用一层
//! 类型擦除硬凑成一个。阶段三方案 B 把用户代码从运行时内部提到驱动循环之后，
//! 驱动本来就知道节点 id、也拿得到旧值，于是可以直接分派 ——
//! [`Computation`](crate::internal::value::Computation) 一个两变体的枚举就够了，
//! memo 的闭包交给 `MemoThunk`（也就是一个普通的 `ThunkBox`）自己装。

use crate::{
    DerivedId, MemoId,
    internal::value::MemoThunk,
    runtime::{RUNTIME, Runtime},
};

/// 创建一个惰性求值、带相等性门控的派生节点。
///
/// - **惰性**：依赖变化只把它标脏，真正的重算发生在下一次读取（或下游 effect
///   被调度）时。
/// - **门控**：重算后用 `PartialEq` 与旧值比较，只有真的变了才通知下游。
///   一条 memo 链因此能把上游的抖动挡在中途。
/// - 计算闭包拿到的 `Option<&T>` 是**上一次的结果**，首次计算时为 `None`。
///   它是借来的，不是克隆来的 —— 需要拥有一份请自己 `clone`（AUDIT P9）。
///
/// # 契约
///
/// 不允许在 `f` 内部读取这个 memo 自己：旧值在 `f` 执行期间被移出了节点，
/// 此时节点里是空的，读它拿到的是 `Reentrant`。旧值请从参数拿。
#[track_caller]
pub fn create<T, F>(f: F) -> MemoId
where
    T: Clone + PartialEq + 'static,
    F: Fn(Option<&T>) -> T + 'static,
{
    MemoId::from_raw(init(MemoThunk::new::<T, F>(f)))
}

/// 创建一个惰性求值但**不做相等性门控**的派生节点。
///
/// 与 [`create`] 的唯一区别就在门控：`T` 只有 `'static` 约束，运行时没有
/// `PartialEq` 可用，因此每一次重算都会通知下游，哪怕算出来的值和上次一样
/// （AUDIT P10）。它换来的是对 `T` 不作任何要求 —— 这正是上层框架的
/// `Signal::derive` 需要的：任意闭包都能包成一个可读节点。
///
/// 值本身仍然是缓存的：没有依赖变化时读它不会重新执行闭包。
#[track_caller]
pub fn derived<T: 'static>(f: Box<dyn Fn() -> T>) -> DerivedId {
    DerivedId::from_raw(init(MemoThunk::new_derived(f)))
}

/// 建节点、装闭包、立即完成首算。
///
/// 计算闭包先被装进节点，再由统一的驱动路径跑首算：这样首算与后续重算走
/// 同一条路径，也不存在“闭包尚未被节点接管就提前返回”导致析构函数永不运行的
/// 窗口（AUDIT P19.10）。
#[track_caller]
#[inline(never)]
fn init(thunk: MemoThunk) -> crate::RawNodeId {
    let rt = RUNTIME.get_or(Runtime::new);
    let id = rt.register_node();
    rt.initialize_memo(id, thunk);
    id
}
