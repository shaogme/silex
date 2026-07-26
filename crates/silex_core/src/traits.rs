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
//! 3. **类型擦除 (Type Erasure)**：为了防止泛型导致的单态化代码膨胀 (Monomorphization Bloat)，底层操作借助统一的 `StaticMapPayload`/`UnifiedStaticMapPayload` 技术。
//!    - 复杂的派生和运算符由统一的静态映射载体承载，使用 Const Generics 和函数指针处理不同类型的逻辑，极大减少了二进制体积。
//!
//! ## 元组与组合性
//!
//! 通过宏自动实现，Silex 支持将包含多个响应式值的元组（如 `(Signal<A>, Signal<B>)`）转换为单一的 `Rx`：
//! - **输入归一化**：元组各元素会通过类型擦除转换为节点 ID，由统一的静态映射载体追踪和驱动。
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
//! 这些运算通过统一的静态映射载体在不带来大量单态化代码膨胀的情况下流畅返回组合的派生 `Rx`。

#[doc(hidden)]
pub mod adaptive;

mod base_impls;
mod guards_impls;
mod list_impls;
mod read_impls;
mod write_impls;

pub use read_impls::{create_tuple_n_rx, create_tuple2_rx};

use std::{fmt::Debug, panic::Location, rc::Rc};

use crate::{
    SilexError,
    error::SilexResult,
    reactivity::{Memo, NodeId, Signal},
    traits::adaptive::{AdaptiveFallback, AdaptiveWrapper},
};

// ==========================================
// 1. 核心数据约束与 Traits 定义
// ==========================================

/// 框架数据约束聚合层，用于统一管理生命周期与能力要求。
pub trait RxData: 'static {}
impl<T: ?Sized + 'static> RxData for T {}

pub trait RxCloneData: Clone + RxData {}
impl<T: Clone + 'static> RxCloneData for T {}

pub trait RxError: Clone + Debug + RxData {}
impl<T: Clone + Debug + 'static> RxError for T {}

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
        // 默认实现只知道“这是一个可读节点”，种类信息在各实现者手里。
        self.id()
            .map(|id| !::silex_reactivity::SignalId::from_raw_unchecked(id).is_alive())
            .unwrap_or(false)
    }

    /// 源码定义位置，用于调试模式下的错误追踪。
    fn defined_at(&self) -> Option<&'static Location<'static>>;

    /// 调试名称（由 `.with_name()` 设置）。
    fn debug_name(&self) -> Option<String> {
        None
    }
}

// ==========================================
// 2. Guards 结构体与 Trait
// ==========================================

/// 内部辅助 Trait，用于抹平所有权存储与借用目标之间的 Deref 差异。
pub trait GuardStorage<T: ?Sized> {
    fn borrow_storage(&self) -> &T;
}

/// 统一大一统的响应式守卫。
///
/// - 'a: 借用生命周期。
/// - T: 逻辑值类型（支持 ?Sized）。
/// - S: 内部存储类型（必须 Sized，默认为 ()，当需要 Owned 变体时应指定具体的类型）。
pub enum RxGuard<'a, T: ?Sized, S = ()> {
    /// 借用变体：可以是来自 Arena 的信号引用，也可以是来自 Constant 的静态引用。
    Borrowed {
        value: &'a T,
        /// 只是一个来源标记（防止把 guard 的生命周期跟丢），不参与任何解引用。
        /// 从前这里放的是 `NodeRef<()>` —— 一个把**任意**节点 id 装进
        /// “node-ref 句柄” 里的类型混淆（审计报告 §3.1）。
        token: Option<NodeId>,
    },
    /// 所有权变体：持有计算结果或内联值。
    Owned(S),
}

// ==========================================
// 3. 读取相关 Traits
// ==========================================

/// 允许将各种类型（原始类型、信号、Rx）转换为统一的 `Rx` 包装器。
///
/// *注意*: 原始类型（i32, f64, &str 等）会自动转换为 `Constant<T>`。
pub trait IntoRx: RxValue {
    type RxType;
    fn into_rx(self) -> Self::RxType;
    fn is_constant(&self) -> bool;
}

/// 将任何响应式类型强转为完全归一化的 `Signal<T>` 枚举。
/// 这是 Silex 内部实现零成本类型擦除的核心机制。
pub trait IntoSignal: RxValue {
    fn into_signal(self) -> Signal<Self::Value>
    where
        Self: Sized + RxData,
        Self::Value: Sized + RxCloneData;
}

/// A trait used internally by `Rx` to delegate calls to either a closure or a reactive primitive.
#[doc(hidden)]
pub trait RxInternal: RxBase {
    /// 自适应返回类型：由具体实现决定返回 Borrowed 或 Owned
    type ReadOutput<'a>
    where
        Self: 'a;

    /// 响应式读取：追踪依赖并返回守卫。
    #[inline(always)]
    fn rx_read(&self) -> Option<Self::ReadOutput<'_>> {
        self.track();
        self.rx_read_untracked()
    }

    /// 非响应式读取：不追踪依赖并返回守卫。
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>>;

    /// 提供对值的闭包式不可变访问（不追踪依赖）。
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    fn rx_is_constant(&self) -> bool {
        false
    }

    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| AdaptiveWrapper(v).maybe_clone())
            .flatten()
    }
}

#[doc(hidden)]
/// Provides a sensible panic message for accessing disposed reactive values.
#[macro_export]
macro_rules! unwrap_rx {
    ($rx:ident) => {{
        let defined_at = $rx.defined_at();
        let debug_name = $rx.debug_name();
        let location = std::panic::Location::caller();
        move || {
            $crate::reactivity::dispatch::report_disposed(defined_at, debug_name, location);
        }
    }};
}

/// 统一的自适应读取与访问 Trait (Unified Read and Access)。
/// 向上统一 Guard 访问机制（借用）和闭包访问机制（映射），
/// 用户无需关心底层是克隆还是借用，自动根据类型智能提供最合适的方式。
pub trait RxRead: RxInternal {
    /// 执行响应式读取，返回一个智能守卫。
    #[track_caller]
    fn read(&self) -> Self::ReadOutput<'_> {
        self.try_read().unwrap_or_else(unwrap_rx!(self))
    }

    /// 执行响应式读取，返回一个智能守卫。如果信号已被销毁，返回 `None`。
    #[track_caller]
    fn try_read(&self) -> Option<Self::ReadOutput<'_>> {
        self.rx_read()
    }

    /// 执行非响应式读取，返回一个智能守卫。
    #[track_caller]
    fn read_untracked(&self) -> Self::ReadOutput<'_> {
        self.try_read_untracked().unwrap_or_else(unwrap_rx!(self))
    }

    /// 执行非响应式读取，返回一个智能守卫。如果信号已被销毁，返回 `None`。
    #[track_caller]
    fn try_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.rx_read_untracked()
    }

    /// 响应式读取：订阅更改，并通过闭包访问底层值，返回闭包执行的结果。
    #[track_caller]
    fn with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with(fun).unwrap_or_else(unwrap_rx!(self))
    }

    /// 响应式读取：订阅更改，并通过闭包访问底层值。如果信号已被销毁，返回 `None`。
    #[track_caller]
    fn try_with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.track();
        self.rx_try_with_untracked(fun)
    }

    /// 非响应式读取：通过闭包访问底层值（不订阅），返回闭包执行的结果。
    #[track_caller]
    fn with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with_untracked(fun)
            .unwrap_or_else(unwrap_rx!(self))
    }

    /// 非响应式读取：通过闭包访问底层值（不订阅）。如果信号已被销毁，返回 `None`。
    #[track_caller]
    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.rx_try_with_untracked(fun)
    }

    /// 尝试获取值的副本。该方法不强制要求 `Clone` 约束（自适应回退）。
    /// - 如果信号已销毁 / 未实现 Clone：返回 `None`。
    #[track_caller]
    fn try_get_cloned(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.track();
        self.rx_get_adaptive()
    }

    /// 非响应式地尝试获取值的副本（自适应回退）。
    #[track_caller]
    fn try_get_cloned_untracked(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_get_adaptive()
    }

    /// 获取值的副本或默认值。如果不支持克隆或信号已销毁，返回 `Default::default()`。
    #[track_caller]
    fn get_cloned_or_default(&self) -> Self::Value
    where
        Self::Value: Sized + Default,
    {
        self.try_get_cloned().unwrap_or_default()
    }
}

// ==========================================
// 响应式 Option 特化扩展 (RxOptionExt)
// ==========================================

/// 当响应式类型持有的值为 `Option<T>` 时的特化扩展能力。
pub trait RxOptionExt<T>: RxRead<Value = Option<T>> {
    /// 响应式映射：从内部 `Option<T>` 中映射值，并在为 `None` 时回退到默认值，返回一个全新的派生 `Memo<U>`。
    fn map_or<U>(&self, default: U, f: impl Fn(&T) -> U + 'static) -> Memo<U>
    where
        Self: Clone + 'static,
        U: PartialEq + Clone + 'static,
    {
        let this = self.clone();
        Memo::new(move |_| {
            this.with(|opt| opt.as_ref().map(&f))
                .unwrap_or_else(|| default.clone())
        })
    }

    /// 响应式映射 (Closure fallback)：使用 Closure 延迟计算 `None` 时的回退默认值。
    fn map_or_else<U>(
        &self,
        default: impl Fn() -> U + 'static,
        f: impl Fn(&T) -> U + 'static,
    ) -> Memo<U>
    where
        Self: Clone + 'static,
        U: PartialEq + Clone + 'static,
    {
        let this = self.clone();
        Memo::new(move |_| {
            this.with(|opt| opt.as_ref().map(&f))
                .unwrap_or_else(&default)
        })
    }

    /// 响应式解包或回退 (`Option<T>` -> `Memo<T>`)。
    fn unwrap_or(&self, default: T) -> Memo<T>
    where
        Self: Clone + 'static,
        T: PartialEq + Clone + 'static,
    {
        self.map_or(default, |v| v.clone())
    }

    /// 校验内部 `Option<T>` 是否为 `Some` 且满足谓词条件。
    fn is_some_and(&self, f: impl Fn(&T) -> bool + 'static) -> Memo<bool>
    where
        Self: Clone + 'static,
    {
        self.map_or(false, f)
    }

    /// 响应式 `Option::and_then` 映射，返回派生 `Memo<Option<U>>`。
    fn and_then<U>(&self, f: impl Fn(&T) -> Option<U> + 'static) -> Memo<Option<U>>
    where
        Self: Clone + 'static,
        U: PartialEq + Clone + 'static,
    {
        let this = self.clone();
        Memo::new(move |_| this.with(|opt| opt.as_ref().and_then(&f)))
    }

    /// 若底层值为 `Some(T)`，非响应式地执行闭包副作用，返回闭包的执行结果。
    fn if_some_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.with_untracked(|opt| opt.as_ref().map(f))
    }
}

impl<S, T> RxOptionExt<T> for S where S: RxRead<Value = Option<T>> {}

/// 克隆获取特质。仅当值支持克隆时自动生效。
/// 该 Trait 仅包含接口定义，具体的 HRTB 约束延迟到 Blanket Implementation 中处理，
/// 从而简化了用户在使用该 Trait 作为约束时的书写负担。
pub trait RxGet: RxRead
where
    Self::Value: Clone + Sized,
{
    /// 非响应式地克隆和返回值。如果是被销毁的，返回 None。
    fn try_get_untracked(&self) -> Option<Self::Value>;

    /// 非响应式地克隆和返回值。
    fn get_untracked(&self) -> Self::Value;

    /// 响应式地订阅信号，克隆并返回值。已被销毁则返回 None。
    fn try_get(&self) -> Option<Self::Value>;

    /// 响应式地订阅信号，克隆并返回值。
    fn get(&self) -> Self::Value;
}

// ==========================================
// 4. 写入与通知 Trait
// ==========================================

/// 统一写入与通知 Trait (Unified Write and Notification).
/// 向上整合了所有更新、替换及通知机制，开发者只需实现最基础的闭包突变和通知接口。
pub trait RxWrite: RxBase {
    /// 仅应用可变闭包更变数据，不通知任何订阅者。（底层无感更新）
    /// 如果目标已被 disposed，则返回 None。
    fn rx_try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet>;

    /// 手动向所有依赖此节点的订阅者发送数据变更通知。
    fn rx_notify(&self);

    // ==========================================
    // 便利 Blanket API (由框架提供默认实现)
    // ==========================================

    /// 响应式更新：使用闭包就地修改数据，并在完成后触发通知。
    #[track_caller]
    fn update(&self, fun: impl FnOnce(&mut Self::Value)) {
        self.try_update(fun).unwrap_or_else(unwrap_rx!(self))
    }

    /// 尝试响应式更新：被销毁时返回 None。
    #[track_caller]
    fn try_update<URet>(&self, fun: impl FnOnce(&mut Self::Value) -> URet) -> Option<URet> {
        let res = self.rx_try_update_untracked(fun)?;
        self.rx_notify();
        Some(res)
    }

    /// 响应式替换：直接用新数据覆盖原有的值，然后触发通知。
    #[track_caller]
    fn set(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update(|v| *v = value);
    }

    /// 尝试响应式替换。
    #[track_caller]
    fn try_set(&self, value: Self::Value) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        if self.is_disposed() {
            Some(value)
        } else {
            self.set(value);
            None
        }
    }

    /// 根据条件触发修改与通知。闭包返回 true 才会触发 notify。
    #[track_caller]
    fn maybe_update(&self, fun: impl FnOnce(&mut Self::Value) -> bool) {
        if let Some(should_notify) = self.rx_try_update_untracked(fun)
            && should_notify
        {
            self.rx_notify();
        }
    }

    /// 静默更新：使用闭包就地修改数据，但【不触发通知】。
    #[track_caller]
    fn update_untracked<URet>(&self, fun: impl FnOnce(&mut Self::Value) -> URet) -> URet {
        self.rx_try_update_untracked(fun)
            .unwrap_or_else(unwrap_rx!(self))
    }

    /// 尝试静默更新。
    #[track_caller]
    fn try_update_untracked<URet>(
        &self,
        fun: impl FnOnce(&mut Self::Value) -> URet,
    ) -> Option<URet> {
        self.rx_try_update_untracked(fun)
    }

    /// 静默替换：直接覆写新数据，【不触发通知】。
    #[track_caller]
    fn set_untracked(&self, value: Self::Value)
    where
        Self::Value: Sized,
    {
        self.update_untracked(|v| *v = value);
    }

    /// 尝试静默替换。
    #[track_caller]
    fn try_set_untracked(&self, value: Self::Value) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        if self.is_disposed() {
            Some(value)
        } else {
            self.set_untracked(value);
            None
        }
    }

    /// 独立触发变更通知。
    #[track_caller]
    fn notify(&self) {
        self.rx_notify();
    }

    /// 返回一个闭包，调用时会将信号设置为指定值。
    fn setter(self, value: Self::Value) -> impl Fn() + Clone + 'static
    where
        Self: Sized + Clone + 'static,
        Self::Value: Sized + Clone,
    {
        move || self.set(value.clone())
    }

    /// 返回一个闭包，调用时会使用提供的函数更新信号。
    fn updater<F>(self, f: F) -> impl Fn() + Clone + 'static
    where
        Self: Sized + Clone + 'static,
        Self::Value: Sized,
        F: Fn(&mut Self::Value) + Clone + 'static,
    {
        move || self.update(f.clone())
    }
}

// ==========================================
// 5. 列表 Iterator 与 Error Handler Traits
// ==========================================

/// Trait to unify different types of data sources that can be used in a `For` loop
/// via zero-copy slice access.
pub trait ForLoopSource {
    type Item: Clone;

    /// Returns a slice of the items.
    fn as_slice(&self) -> SilexResult<&[Self::Item]>;
}

#[derive(Clone)]
pub struct ForErrorHandler(Rc<dyn Fn(SilexError)>);
