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

use crate::reactivity::{DerivedPayload, Memo, NodeId};
use std::ops::Deref;
use std::panic::Location;

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
pub mod impls;

#[doc(hidden)]
pub mod guards;
#[doc(hidden)]
pub use guards::*;

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

/// 允许将各种类型（原始类型、信号、Rx）转换为统一的 `Rx` 包装器。
///
/// *注意*: 原始类型（i32, f64, &str 等）会自动转换为 `Constant<T>`。
pub trait IntoRx {
    type Value: ?Sized;
    type RxType;
    fn into_rx(self) -> Self::RxType;
    fn is_constant(&self) -> bool;

    /// 将当前类型转换为归一化后的 Signal<T>。
    /// 这是 Silex 内部实现零成本类型擦除的核心机制。
    fn into_signal(self) -> crate::reactivity::Signal<Self::Value>
    where
        Self: Sized + 'static,
        Self::Value: Sized + Clone + 'static;
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

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        false
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }
}

pub type CompareFn<T> = fn(&T, &T) -> bool;

#[doc(hidden)]
macro_rules! reactive_compare_method {
    ($name:ident, $fn_impl:ident, $op:tt, $bound:ident) => {
        fn $name<O>(
            &self,
            other: O,
        ) -> $crate::Rx<$crate::reactivity::OpPayload<bool>, $crate::RxValue>
        where
            Self: IntoRx,
            Self::RxType: RxInternal<Value = <Self as IntoRx>::Value> + Clone + 'static,
            O: IntoRx<Value = <Self as IntoRx>::Value> + 'static,
            O::RxType: RxInternal<Value = <Self as IntoRx>::Value> + Clone + 'static,
            <Self as IntoRx>::Value: $bound + Sized + Clone + 'static,
        {
            let lhs = self.clone().into_signal();
            let rhs = other.into_signal();

            #[inline(always)]
            unsafe fn read_impl<InnerT: $bound + 'static>(inputs: &[NodeId]) -> Option<bool> {
                unsafe {
                    let a = $crate::reactivity::rx_borrow_signal_unsafe::<InnerT>(inputs[0])?;
                    let b = $crate::reactivity::rx_borrow_signal_unsafe::<InnerT>(inputs[1])?;
                    Some($crate::traits::impls::ops_impl::$fn_impl(a, b))
                }
            }

            let is_const = lhs.is_constant() && rhs.is_constant();

            $crate::Rx(
                $crate::reactivity::OpPayload {
                    inputs: [lhs.ensure_node_id(), rhs.ensure_node_id()],
                    input_count: 2,
                    read: read_impl::<<Self as IntoRx>::Value>,
                    track: $crate::reactivity::op_trampolines::track_inputs,
                    is_constant: is_const,
                },
                ::core::marker::PhantomData,
            )
        }
    };
}

/// Provides a fluent API for checking equality on reactive values.
pub trait ReactivePartialEq: RxRead + Clone + 'static
where
    Self::Value: PartialEq + Sized + 'static,
    for<'a> Self::ReadOutput<'a>: Deref<Target = Self::Value>,
{
    reactive_compare_method!(equals, eq, ==, PartialEq);
    reactive_compare_method!(not_equals, ne, !=, PartialEq);
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: RxRead + Clone + 'static
where
    Self::Value: PartialOrd + Sized + 'static,
    for<'a> Self::ReadOutput<'a>: Deref<Target = Self::Value>,
{
    reactive_compare_method!(greater_than, gt, >, PartialOrd);
    reactive_compare_method!(less_than, lt, <, PartialOrd);
    reactive_compare_method!(greater_than_or_equals, ge, >=, PartialOrd);
    reactive_compare_method!(less_than_or_equals, le, <=, PartialOrd);
}

#[doc(hidden)]
/// Provides a sensible panic message for accessing disposed reactive values.
#[macro_export]
macro_rules! unwrap_rx {
    ($rx:ident) => {{
        #[cfg(debug_assertions)]
        let location = std::panic::Location::caller();
        move || {
            #[cfg(debug_assertions)]
            {
                panic!(
                    "{}",
                    $crate::traits::panic_getting_disposed_signal(
                        $rx.defined_at(),
                        $rx.debug_name(),
                        location
                    )
                );
            }
            #[cfg(not(debug_assertions))]
            {
                panic!(
                    "Tried to access a reactive value that has already been \
                     disposed."
                );
            }
        }
    }};
}

/// 统一的自适应读取与访问 Trait (Unified Read and Access)。
/// 向上统一 Guard 访问机制（借用）和闭包访问机制（映射），
/// 用户无需关心底层是克隆还是借用，自动根据类型智能提供最合适的方式。
pub trait RxRead: RxInternal
where
    for<'a> Self::ReadOutput<'a>: Deref<Target = Self::Value>,
{
    // ==========================================
    // 1. Guard 方式的访问（原 Read trait 功能）
    // ==========================================

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

    // ==========================================
    // 2. 闭包方式的访问（原 With/WithUntracked trait 功能）
    // ==========================================

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

    // ==========================================
    // 3. 克隆获取（Getters）
    // ==========================================

    /// 非响应式地克隆和返回值。如果是被销毁的，返回 None。
    #[track_caller]
    fn try_get_untracked(&self) -> Option<Self::Value>
    where
        Self::Value: Sized + Clone,
    {
        self.try_read_untracked().map(|v| v.clone())
    }

    /// 非响应式地克隆和返回值。
    ///
    /// # Panics
    /// 访问被销毁的信号时报错。
    #[track_caller]
    fn get_untracked(&self) -> Self::Value
    where
        Self::Value: Sized + Clone,
    {
        self.try_get_untracked()
            .unwrap_or_else(|| unwrap_rx!(self)())
    }

    /// 响应式地订阅信号，克隆并返回值。已被销毁则返回 None。
    #[track_caller]
    fn try_get(&self) -> Option<Self::Value>
    where
        Self::Value: Sized + Clone,
    {
        self.try_read().map(|v| v.clone())
    }

    /// 响应式地订阅信号，克隆并返回值。
    ///
    /// # Panics
    /// 访问被销毁的信号时报错。
    #[track_caller]
    fn get(&self) -> Self::Value
    where
        Self::Value: Sized + Clone,
    {
        self.try_get().unwrap_or_else(|| unwrap_rx!(self)())
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

impl<T: ?Sized + RxInternal> RxRead for T where for<'a> T::ReadOutput<'a>: Deref<Target = T::Value> {}

/// Allows disposing an arena-allocated signal before its owner has been disposed.
pub trait Dispose {
    /// Disposes of the signal. This:
    /// 1. Detaches the signal from the reactive graph, preventing it from triggering
    ///    further updates; and
    /// 2. Drops the value contained in the signal.
    fn dispose(self);
}

/// Allows creating a derived signal from this signal.
///
/// Unlike [`Get`], this trait uses closure-based access as its basis, meaning it works
/// with the zero-copy access pattern.
pub trait Map: RxBase + Sized {
    /// Creates a derived signal from this signal.
    fn map<U, F>(self, f: F) -> crate::Rx<DerivedPayload<Self, F>, crate::RxValue>
    where
        F: Fn(&Self::Value) -> U + Clone + 'static;
}

/// Allows converting a signal into a memoized signal.
///
/// Requires `Value: Clone + Sized` since memoization needs to clone and store values.
pub trait Memoize: RxRead + Clone + 'static
where
    Self::Value: Sized,
    for<'a> Self::ReadOutput<'a>: Deref<Target = Self::Value>,
{
    /// Memoizes the value of the signal.
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: Clone + PartialEq + 'static;
}

/// Notifies subscribers of a change in this signal.
pub trait Notify {
    /// Notifies subscribers of a change in this signal.
    #[track_caller]
    fn notify(&self);
}

/// Updates the value of a signal by applying a function that updates it in place,
/// without notifying subscribers.
pub trait UpdateUntracked: RxBase {
    /// Updates the value by applying a function, returning the value returned by that function.
    /// Does not notify subscribers that the signal has changed.
    ///
    /// # Panics
    /// Panics if you try to update a signal that has been disposed.
    #[track_caller]
    fn update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> U {
        self.try_update_untracked(fun)
            .unwrap_or_else(unwrap_rx!(self))
    }

    /// Updates the value by applying a function, returning the value returned by that function,
    /// or `None` if the signal has already been disposed.
    /// Does not notify subscribers that the signal has changed.
    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>;
}

/// Updates the value of a signal by applying a function that updates it in place,
/// notifying its subscribers that the value has changed.
pub trait Update: UpdateUntracked + Notify {
    /// Updates the value of the signal and notifies subscribers.
    #[track_caller]
    fn update(&self, fun: impl FnOnce(&mut Self::Value)) {
        self.try_update(fun);
    }

    /// Updates the value of the signal, but only notifies subscribers if the function
    /// returns `true`.
    #[track_caller]
    fn maybe_update(&self, fun: impl FnOnce(&mut Self::Value) -> bool) {
        self.try_maybe_update(|val| {
            let did_update = fun(val);
            (did_update, ())
        });
    }

    /// Updates the value of the signal and notifies subscribers, returning the value that is
    /// returned by the update function, or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        self.try_maybe_update(|val| (true, fun(val)))
    }

    /// Updates the value of the signal, notifying subscribers if the update function returns
    /// `(true, _)`, and returns the value returned by the update function,
    /// or `None` if the signal has already been disposed.
    fn try_maybe_update<U>(&self, fun: impl FnOnce(&mut Self::Value) -> (bool, U)) -> Option<U>;
}

/// Updates the value of the signal by replacing it.
pub trait Set: Update
where
    Self::Value: Sized,
{
    /// Updates the value by replacing it.
    #[track_caller]
    fn set(&self, value: Self::Value);

    /// Updates the value by replacing it.
    ///
    /// If the signal has already been disposed, returns `Some(value)` with the value that was
    /// passed in. Otherwise, returns `None`.
    fn try_set(&self, value: Self::Value) -> Option<Self::Value>;
}

/// Allows creating a setter closure from this signal.
pub trait SignalSetter: RxBase + Sized
where
    Self::Value: Sized,
{
    /// Returns a closure that sets the signal's value when called.
    fn setter(self, value: Self::Value) -> impl Fn() + Clone + 'static;
}

/// Allows creating an updater closure from this signal.
pub trait SignalUpdater: RxBase + Sized
where
    Self::Value: Sized,
{
    /// Returns a closure that updates the signal's value using the provided function when called.
    fn updater<F>(self, f: F) -> impl Fn() + Clone + 'static
    where
        F: Fn(&mut Self::Value) + Clone + 'static;
}

#[doc(hidden)]
pub fn panic_getting_disposed_signal(
    defined_at: Option<&'static Location<'static>>,
    debug_name: Option<String>,
    location: &'static Location<'static>,
) -> String {
    if let Some(name) = debug_name {
        if let Some(defined_at) = defined_at {
            format!(
                "At {location}, you tried to access a reactive value \"{name}\" which was \
                 defined at {defined_at}, but it has already been disposed."
            )
        } else {
            format!(
                "At {location}, you tried to access a reactive value \"{name}\", but it has \
                 already been disposed."
            )
        }
    } else if let Some(defined_at) = defined_at {
        format!(
            "At {location}, you tried to access a reactive value which was \
             defined at {defined_at}, but it has already been disposed."
        )
    } else {
        format!(
            "At {location}, you tried to access a reactive value, but it has \
             already been disposed."
        )
    }
}

/// Updates the value of the signal by replacing it, without notifying subscribers.
pub trait SetUntracked: RxBase
where
    Self::Value: Sized,
{
    /// Updates the value by replacing it, non-reactively.
    ///
    /// If the signal has already been disposed, returns `Some(value)` with the value that was
    /// passed in. Otherwise, returns `None`.
    fn try_set_untracked(&self, value: Self::Value) -> Option<Self::Value>;

    /// Updates the value by replacing it, non-reactively.
    ///
    /// # Panics
    /// Panics if you try to set a signal that has been disposed.
    #[track_caller]
    fn set_untracked(&self, value: Self::Value) {
        if self.try_set_untracked(value).is_some() {
            panic!(
                "{}",
                crate::traits::panic_getting_disposed_signal(
                    self.defined_at(),
                    self.debug_name(),
                    std::panic::Location::caller()
                )
            );
        }
    }
}
