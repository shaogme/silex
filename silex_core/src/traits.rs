//! 定义响应式原始组件（Signals, Memos, Rx 等）的行为核心。
//!
//! ## 设计哲学：零拷贝优先、Rx 委托与类型擦除
//!
//! Silex 的 Trait 系统建立在以下三个核心理念之上：
//!
//! 1. **零拷贝 (Zero-Copy)**：闭包读取是系统的第一公民。
//!    - 信号值通常存储在 Arena（Map/Vec）中。访问它们涉及获取锁。
//!    - 使用闭包允许直接处理 `&T`，避免了不必要的 `Clone` 开销。
//!    - 天然支持动态大小类型（DST），如 `str` 或 `[T]`。
//! 2. **Rx 委托 (Rx Delegate)**：现代 Silex 采用委托模式。[`Rx`] 包装器是所有响应式操作（算术、比较、Map 等）的对外接口。
//!    - 通过 [`IntoRx`] 接口，常量、信号、闭包及元组都能无缝转化为统一的 `Rx`。
//! 3. **类型擦除 (Type Erasure)**：为了防止泛型导致的单态化代码膨胀 (Monomorphization Bloat)，底层操作借助响应式归一化技术。
//!    - 复杂的派生和运算符现在被转化并由统一的 `Signal<T>` 和 `DerivedPayload` 处理，消除了大量针对性的组合结构泛型声明代码。
//!
//! ## 元组与组合性
//!
//! 通过宏自动实现，Silex 支持将包含多个响应式值的元组（如 `(Signal<A>, Signal<B>)`）转换为单一的 `Rx`：
//! - **派生聚合**：元组各元素会通过类型擦除转换为 `Signal<T>`，并由统一的 `DerivedPayload` 承载，消除了大量针对性的组合结构泛型声明代码。
//! - **组合评估**：转换会隐式克隆各元素重新构建结果元组。
//! - **零拷贝路径**：对于追求极致性能的多路且无需克隆的场景，依然推荐使用 [`batch_read!`] 宏，它提供了真实的底层结构分段式借用多路读取。
//!
//! ## 核心原则
//! 1. **组合语义**：大多数高级 Trait（如 `RxRead`, `Map`）都是通过基础 Trait 组合而成的 Blanket Implementations。
//! 2. **原子化实现**：底层原语只需实现 [`Track`], [`Notify`] 和 [`UpdateUntracked`]。
//! 3. **自适应读取**：[`RxRead`] Trait 统一了响应式与非响应式读取，并提供可选的克隆访问（`get` 系列方法）。
//! 4. **容错性**：大多数读取操作包含 `try_` 变体，在信号已被销毁（Disposed）时安全返回 `None`。
//!
//! ## Trait 结构一览
//!
//! ### 基础核心 (底层实现者关注)
//! | Trait               | 作用                                                                           |
//! |---------------------|--------------------------------------------------------------------------------|
//! | [`Track`]           | 追踪变化，将当前值添加为当前响应式上下文的依赖。                               |
//! | [`Notify`]          | 通知订阅者该值已更改。                                                         |
//! | [`UpdateUntracked`] | 提供闭包式可变更新（不触发通知）。                                            |
//! | [`RxInternal`]      | **内部桥梁**：被包装在 `Rx` 内部的统一操作接口（对用户隐藏）。                |
//!
//! ### 派生能力 (基于自动实现)
//! | 类别 | Trait | 组合方式 | 描述 |
//! |------|-------|---------|------|
//! | **读取** | [`RxRead`] | `RxInternal` | **核心读取接口**：支持自适应读取（Guard/闭包）与克隆读取（`get`）。 |
//! | | [`Map`] | `RxRead` | 创建派生信号（`Derived`），返回 `Rx`。 |
//! | **更新** | [`Update`] | `UpdateUntracked` + `Notify` | 应用闭包并通知系统刷新。 |
//! | | [`Set`] | `Update` | 直接覆盖旧值。 |
//! | **转换** | [`IntoRx`] | — | **大一统接口**：将任意类型转化为统一的 `Rx`。 |
//!
//! ## 比较与算术运算
//!
//! 所有的 `Rx` 类型通过 [`ReactivePartialEq`] 和 [`ReactivePartialOrd`] 获得流畅的比较接口（如 `.equals()`），
//! 并自动支持标准算术运算符（`+`, `-`, `*`, `/` 等），这些运算的底层实现通过宏和擦除技术，能在不带来大量单态化代码膨胀的情况下流畅返回组合的派生 `Rx`。
//!
//! ## 多信号访问示例
//!
//! 使用 [`batch_read!`] 宏实现零拷贝多信号访问：
//!
//! ```rust,ignore
//! let (name, age) = (signal("Alice".to_string()), signal(42));
//!
//! // 零拷贝：直接通过引用访问，不克隆 String
//! batch_read!(name, age => |name: &String, age: &i32| {
//!     println!("{} is {} years old", name, age);
//! });
//! ```

use crate::reactivity::NodeId;

/// 响应式系统的基础层级，统一了标识、追踪、生命周期监测和源码定位。
pub trait RxBase {
    /// 响应式值持有的数据类型。支持 ?Sized 以兼容 [T] 或 str。
    type Value: ?Sized;

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
