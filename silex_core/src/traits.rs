//! 定义响应式原始组件（Signals, Memos, Rx 等）的行为核心。
//!
//! ## 设计哲学：零拷贝优先、Rx 委托与类型擦除
//!
//! Silex 的 Trait 系统建立在以下三个核心理念之上：
//!
//! 1. **零拷贝 (Zero-Copy)**：闭包读取与智能守卫是系统的第一公民。
//!    - 信号值通常存储在 Arena 中。使用 [`RxGuard`] 允许直接返回引用（`Borrowed`），避免不必要的 `Clone`。
//!    - 对于计算结果，[`RxGuard`] 支持持有所有权（`Owned`），从而统一了借用与拥有权访问。
//!    - 天然支持动态大小类型（DST），如 `str` 或 `[T]`。
//! 2. **Rx 委托 (Rx Delegate)**：[`Rx`] 包装器是所有响应式操作（算术、比较、Map 等）的对外接口。
//!    - 通过 [`IntoRx`] 接口，常量、信号、闭包及元组都能无缝转化为统一的 `Rx`。
//! 3. **类型擦除 (Type Erasure)**：为了防止泛型导致的单态化代码膨胀 (Monomorphization Bloat)，底层操作借助 `OpPayload<V, N>` 技术。
//!    - 复杂的派生和运算符由统一的 `OpPayload` 承载，使用 Const Generics 指定输入数量，并通过静态函数指针处理不同类型的逻辑，极大减少了二进制体积。
//!
//! ## 元组与组合性
//!
//! 通过宏自动实现，Silex 支持将包含多个响应式值的元组（如 `(Signal<A>, Signal<B>)`）转换为单一的 `Rx`：
//! - **输入归一化**：元组各元素会通过类型擦除转换为节点 ID，由 [`OpPayload`] 统一追踪和驱动。
//! - **聚合读取**：读取时会自动构建结果元组。对于追求极致性能的场景，框架内部使用非克隆路径尽可能减少开销。
//!
//! ## 核心原则
//! 1. **组合语义**：大多数高级 Trait（如 `Map`, `Memoize`, `ReactivePartialEq`）都是通过基础 Trait 组合而成的 Blanket Implementations。
//! 2. **原子化实现**：底层原语只需实现 [`RxBase`] 和 [`RxInternal`]。
//! 3. **统一读取与写入**：[`RxRead`] 统一了守卫式、闭包式访问及克隆获取；[`RxWrite`] 统一了更新、替换与通知。
//! 4. **容错性**：大多数操作包含 `try_` 变体，在信号已被销毁（Disposed）时安全返回 `None`。
//!
//! ## Trait 结构一览
//!
//! ### 基础核心 (底层实现者关注)
//! | Trait          | 作用                                                                           |
//! |----------------|--------------------------------------------------------------------------------|
//! | [`RxBase`]     | **基础层级**：提供 ID、追踪、定义位置及生命周期检查。                        |
//! | [`RxInternal`] | **内部桥梁**：定义响应式读取的底层代理逻辑（对用户隐藏）。                    |
//! | [`RxWrite`]    | **统一写入**：定义基础的闭包突变 (`rx_try_update_untracked`) 和通知逻辑。      |
//!
//! ### 用户接口 (面向开发者)
//! | 类别 | Trait | 描述 |
//! |------|-------|------|
//! | **转换** | [`IntoRx`] | **大一统接口**：将任意类型转化为统一的 `Rx`。 |
//! | | [`IntoSignal`] | **强力归一化接口**：将任意类型展平为 `Signal<T>` 枚举。 |
//! | **读取** | [`RxRead`] | **统一读取**：支持守卫 (`read`)、闭包 (`with`) 与克隆 (`get`)。 |
//! | **更新** | [`RxWrite`] | **便捷更新**：提供 `update`, `set`, `notify` 等高级 API。 |
//! | **逻辑** | [`Map`] | 派生信号能力，返回 `Rx`。 |
//! | | [`Memoize`] | 提供自带缓存的记忆化能力。 |
//!
//! ## 比较与算术运算
//!
//! 所有的 `Rx` 类型通过 [`ReactivePartialEq`] 和 [`ReactivePartialOrd`] 获得流式比较接口（如 `.equals()`），
//! 并自动支持标准算术运算符（`+`, `-`, `*`, `/` 等）。
//!
//! 这些运算通过 [`OpPayload`] 在不带来大量单态化代码膨胀的情况下流畅返回组合的派生 `Rx`。

use crate::reactivity::NodeId;

/// 框架数据约束聚合层，用于统一管理生命周期与能力要求。
pub trait RxData: 'static {}
impl<T: ?Sized + 'static> RxData for T {}

pub trait RxCloneData: Clone + RxData {}
impl<T: Clone + 'static> RxCloneData for T {}

pub trait RxError: Clone + std::fmt::Debug + RxData {}
impl<T: Clone + std::fmt::Debug + 'static> RxError for T {}

/// 响应式实体的核心价值定义。
pub trait RxValue {
    /// 响应式值持有的数据类型。支持 ?Sized 以兼容 [T] 或 str。
    type Value: ?Sized;
}

/// 响应式系统的基础层级，统一了标识、追踪、生命周期监测和源码定位。
pub trait RxBase: RxValue {
    /// 获取底层节点 ID。常量或非节点组件可能返回 None。
    fn id(&self) -> Option<NodeId>;

    /// 建立响应式追踪（将其设为当前 Effect/Memo 的依赖）。
    fn track(&self);

    /// 检查该值是否已被销毁。
    fn is_disposed(&self) -> bool {
        self.id()
            .map(|id| !crate::reactivity::is_signal_valid(id))
            .unwrap_or(false)
    }

    /// 源码定义位置，用于调试模式下的错误追踪。
    fn defined_at(&self) -> Option<&'static std::panic::Location<'static>>;

    /// 调试名称（由 `.with_name()` 设置）。
    fn debug_name(&self) -> Option<String> {
        None
    }
}

#[doc(hidden)]
mod guards;
#[doc(hidden)]
pub use guards::*;

#[macro_use]
mod read;
pub use read::*;

mod write;
pub use write::*;

/// 内部自适应工具，用于在不引入显式 Clone 约束的情况下探测克隆能力。
#[doc(hidden)]
pub mod adaptive {
    pub struct AdaptiveWrapper<'a, T>(pub &'a T);

    impl<'a, T: Clone> AdaptiveWrapper<'a, T> {
        #[inline(always)]
        pub fn maybe_clone(&self) -> Option<T> {
            Some(self.0.clone())
        }
    }

    pub trait AdaptiveFallback {
        type Value;
        fn maybe_clone(&self) -> Option<Self::Value>;
    }

    impl<'a, T> AdaptiveFallback for AdaptiveWrapper<'a, T> {
        type Value = T;
        #[inline(always)]
        fn maybe_clone(&self) -> Option<T> {
            None
        }
    }
}
