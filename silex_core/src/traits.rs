//! 定义响应式原始组件（Signals, Memos, Rx 等）的行为核心。
//!
//! ## 设计哲学：零拷贝优先、Rx 委托与类型擦除
//!
//! Silex 的 Trait 系统建立在以下三个核心理念之上：
//!
//! 1. **零拷贝 (Zero-Copy)**：[`With`]（基于闭包的访问）是系统的第一公民。
//!    - 信号值通常存储在 Arena（Map/Vec）中。访问它们涉及获取锁。
//!    - 使用闭包允许直接处理 `&T`，避免了不必要的 `Clone` 开销。
//!    - 天然支持动态大小类型（DST），如 `str` 或 `[T]`。
//! 2. **Rx 委托 (Rx Delegate)**：现代 Silex 采用委托模式。[`Rx`] 包装器是所有响应式操作（算术、比较、Map 等）的对外接口。
//!    - 通过 [`IntoRx`] 接口，常量、信号、闭包及元组都能无缝转化为统一的 `Rx`。
//! 3. **类型擦除 (Type Erasure)**：为了防止泛型导致的单态化代码膨胀 (Monomorphization Bloat)，底层操作借助 `AnyRxInternal` 进行类型擦除。
//!    - 复杂的派生和运算符现在被转化并由统一的元组派生 `DerivedPayload` 处理共享底层的动态派发机制，极大缩减了编译产物的体积。
//!
//! ## 元组与组合性
//!
//! 通过宏自动实现，Silex 支持将包含多个响应式值的元组（如 `(Signal<A>, Signal<B>)`）转换为单一的 `Rx`：
//! - **派生聚合**：元组各元素会通过类型擦除转换为 `Rc<dyn AnyRxInternal>`，并由统一的 `DerivedPayload` 承载，消除了大量针对性的组合结构泛型声明代码。
//! - **组合评估**：转换会隐式克隆各元素重新构建结果元组。
//! - **零拷贝路径**：对于追求极致性能的多路且无需克隆的场景，依然推荐使用 [`batch_read!`] 宏，它提供了真实的底层结构分段式借用多路读取。
//!
//! ## 核心原则
//! 1. **组合语义**：大多数高级 Trait（如 `With`, `Read`, `Map`）都是通过基础 Trait 组合而成的 Blanket Implementations。
//! 2. **原子化实现**：底层原语只需实现 [`Track`], [`Notify`], [`WithUntracked`] 和 [`UpdateUntracked`]。
//! 3. **自适应读取**：[`Read`] Trait 统一了响应式与非响应式读取，并提供可选的克隆访问（`get` 系列方法）。
//! 4. **容错性**：大多数读取操作包含 `try_` 变体，在信号已被销毁（Disposed）时安全返回 `None`。
//!
//! ## Trait 结构一览
//!
//! ### 基础核心 (底层实现者关注)
//! | Trait               | 作用                                                                           |
//! |---------------------|--------------------------------------------------------------------------------|
//! | [`Track`]           | 追踪变化，将当前值添加为当前响应式上下文的依赖。                               |
//! | [`Notify`]          | 通知订阅者该值已更改。                                                         |
//! | [`WithUntracked`]   | 提供对值的闭包式不可变访问（不追踪依赖）。                                     |
//! | [`UpdateUntracked`] | 提供闭包式可变更新（不触发通知）。                                            |
//! | [`RxInternal`]      | **内部桥梁**：被包装在 `Rx` 内部的统一操作接口（对用户隐藏）。                |
//!
//! ### 派生能力 (基于自动实现)
//! | 类别 | Trait | 组合方式 | 描述 |
//! |------|-------|---------|------|
//! | **读取** | [`With`] | `WithUntracked` + `Track` | 带响应式追踪的闭包式访问（零拷贝）。 |
//! | | [`Read`] | `RxInternal` | **核心读取接口**：支持自适应读取（Guard）与克隆读取（`get`）。 |
//! | | [`Map`] | `With` | 创建派生信号（`Derived`），返回 `Rx`。 |
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
use crate::{NodeRef, Rx, RxValue};
use std::cell::OnceCell;
use std::marker::PhantomData;
use std::ops::Deref;
use std::panic::Location;

/// A internal trait implemented by reactive types that are backed by a `NodeId`.
/// This is used to provide blanket implementations of high-level traits.
#[doc(hidden)]
pub trait ReactivityNode {
    type Value: 'static;
    fn node_id(&self) -> NodeId;
}

#[doc(hidden)]
pub mod impls;

/// 能够持有 Arena 中响应式值引用的守卫。
pub struct RxGuard<'a, T: ?Sized> {
    pub(crate) value: &'a T,
    /// 内部持有 Token 确保数据在 Guard 存续期间不被非法清理
    pub(crate) _guard_token: Option<NodeRef>,
}

impl<'a, T: ?Sized> RxGuard<'a, T> {
    /// 投影守卫持有的引用。
    #[inline(always)]
    pub fn map<U: ?Sized>(self, f: impl FnOnce(&T) -> &U) -> RxGuard<'a, U> {
        RxGuard {
            value: f(self.value),
            _guard_token: self._guard_token,
        }
    }
}

impl<T: ?Sized> Deref for RxGuard<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

/// 能够持有临时生成的响应式值（Owned）的守卫。
pub struct OwnedGuard<T> {
    pub(crate) value: T,
}

impl<T> Deref for OwnedGuard<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// 能够持有静态常量引用的守卫。
pub struct ConstantGuard<T: ?Sized + 'static> {
    pub(crate) value: &'static T,
}

impl<T: ?Sized + 'static> Deref for ConstantGuard<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

macro_rules! impl_tuple_read_guard {
    ($name:ident, $($idx:tt : $meth:ident : $G:ident : $V:ident),+; $cell_idx:tt; $marker_idx:tt) => {
        /// 为元组设计的自适应守卫，支持分段式（零拷贝）读取。
        /// 其结构本身就是一个包含各元素内部守卫的元组，支持通过 `.0`, `.1` 直接访问各分部。
        pub struct $name<'a, $($G, $V),+>(
            $(pub $G,)+
            pub(crate) OnceCell<($($V,)+)>,
            pub(crate) PhantomData<&'a ()>
        );

        impl<'a, $($G, $V),+> $name<'a, $($G, $V),+> {
            $(
                /// 获取该位置的分段守卫引用。
                #[inline(always)]
                pub fn $meth(&self) -> &$G {
                    &self.$idx
                }
            )+
        }

        impl<'a, $($G, $V),+> Deref for $name<'a, $($G, $V),+>
        where
            $($G: Deref<Target = $V>),+,
            $($V: Clone),+
        {
            type Target = ($($V,)+);

            fn deref(&self) -> &Self::Target {
                self.$cell_idx.get_or_init(|| {
                    ($(self.$idx.deref().clone(),)+)
                })
            }
        }
    };
}

impl_tuple_read_guard!(Tuple2ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1; 2; 3);
impl_tuple_read_guard!(Tuple3ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2; 3; 4);
impl_tuple_read_guard!(Tuple4ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2, 3: _3: G3: V3; 4; 5);
impl_tuple_read_guard!(Tuple5ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2, 3: _3: G3: V3, 4: _4: G4: V4; 5; 6);
impl_tuple_read_guard!(Tuple6ReadGuard, 0: _0: G0: V0, 1: _1: G1: V1, 2: _2: G2: V2, 3: _3: G3: V3, 4: _4: G4: V4, 5: _5: G5: V5; 6; 7);

/// 允许将各种类型（原始类型、信号、Rx）转换为统一的 `Rx` 包装器。
///
/// *注意*: 原始类型（i32, f64, &str 等）会自动转换为 `Constant<T>`。
pub trait IntoRx {
    type Value: ?Sized;
    type RxType;
    fn into_rx(self) -> Self::RxType;
    fn is_constant(&self) -> bool;

    /// 将当前类型转换为类型擦除后的 AnyRx。
    fn into_any_rx(self) -> crate::AnyRx<Self::Value>
    where
        Self: Sized,
        Self::Value: Sized + 'static,
        Self::RxType: RxInternal<Value = Self::Value> + Clone + 'static,
    {
        let rx = self.into_rx();
        crate::Rx(rx.rx_into_any(), ::core::marker::PhantomData)
    }
}

/// A trait used internally by `Rx` to delegate calls to either a closure or a reactive primitive.
#[doc(hidden)]
pub trait RxInternal {
    type Value: ?Sized;

    /// 自适应返回类型：由具体实现决定返回 Borrowed 或 Owned
    type ReadOutput<'a>: Deref<Target = Self::Value>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_track(&self) {}

    /// 响应式读取：追踪依赖并返回守卫。
    #[inline(always)]
    fn rx_read(&self) -> Option<Self::ReadOutput<'_>> {
        self.rx_track();
        self.rx_read_untracked()
    }

    /// 非响应式读取：不追踪依赖并返回守卫。
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>>;

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.rx_read_untracked().map(|g| fun(&*g))
    }

    #[inline(always)]
    fn rx_defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    #[inline(always)]
    fn rx_debug_name(&self) -> Option<String> {
        None
    }

    #[inline(always)]
    fn rx_is_disposed(&self) -> bool {
        false
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        false
    }

    /// 将当前的响应式单体转换为类型擦除后的 Rc。
    fn rx_into_any(self) -> std::rc::Rc<dyn AnyRxInternal<Self::Value>>
    where
        Self: Sized + Clone + 'static,
        Self::Value: Sized + 'static,
    {
        std::rc::Rc::new(self) as std::rc::Rc<dyn AnyRxInternal<Self::Value>>
    }
}

/// A type-erased version of [`RxInternal`] to stop monomorphization bloat.
#[doc(hidden)]
pub trait AnyRxInternal<V: ?Sized> {
    fn rx_track_erased(&self);
    fn rx_read_erased(&self) -> Option<ErasedRxGuard<'_, V>>;
    fn rx_read_untracked_erased(&self) -> Option<ErasedRxGuard<'_, V>>;
    fn rx_defined_at_erased(&self) -> Option<&'static std::panic::Location<'static>>;
    fn rx_debug_name_erased(&self) -> Option<String>;
    fn rx_is_disposed_erased(&self) -> bool;
    fn rx_is_constant_erased(&self) -> bool;
}

/// A guard for type-erased reactive values.
pub struct ErasedRxGuard<'a, V: ?Sized> {
    inner: Box<dyn std::ops::Deref<Target = V> + 'a>,
}

impl<V: ?Sized> std::ops::Deref for ErasedRxGuard<'_, V> {
    type Target = V;

    #[inline(always)]
    fn deref(&self) -> &V {
        &self.inner
    }
}

impl<T, V> AnyRxInternal<V> for T
where
    T: RxInternal<Value = V>,
    V: ?Sized,
{
    #[inline(always)]
    fn rx_track_erased(&self) {
        self.rx_track();
    }

    #[inline(always)]
    fn rx_read_erased(&self) -> Option<ErasedRxGuard<'_, V>> {
        self.rx_read().map(|g| ErasedRxGuard {
            inner: Box::new(g) as Box<dyn std::ops::Deref<Target = V>>,
        })
    }

    #[inline(always)]
    fn rx_read_untracked_erased(&self) -> Option<ErasedRxGuard<'_, V>> {
        self.rx_read_untracked().map(|g| ErasedRxGuard {
            inner: Box::new(g) as Box<dyn std::ops::Deref<Target = V>>,
        })
    }

    #[inline(always)]
    fn rx_defined_at_erased(&self) -> Option<&'static std::panic::Location<'static>> {
        self.rx_defined_at()
    }

    #[inline(always)]
    fn rx_debug_name_erased(&self) -> Option<String> {
        self.rx_debug_name()
    }

    #[inline(always)]
    fn rx_is_disposed_erased(&self) -> bool {
        self.rx_is_disposed()
    }

    #[inline(always)]
    fn rx_is_constant_erased(&self) -> bool {
        self.rx_is_constant()
    }
}

pub type CompareFn<T> = fn(&T, &T) -> bool;

#[doc(hidden)]
macro_rules! reactive_compare_method {
    ($name:ident, $op:tt, $bound:ident) => {
        fn $name<O>(
            &self,
            other: O,
        ) -> Rx<
            crate::reactivity::DerivedPayload<(Self::RxType, O::RxType), fn(&(<Self as IntoRx>::Value, <Self as IntoRx>::Value)) -> bool>,
            RxValue,
        >
        where
            Self: IntoRx,
            Self::RxType: RxInternal<Value = <Self as IntoRx>::Value> + Clone + 'static,
            O: IntoRx<Value = <Self as IntoRx>::Value>,
            O::RxType: RxInternal<Value = <Self as IntoRx>::Value> + Clone + 'static,
            <Self as IntoRx>::Value: $bound + Sized + 'static,
        {
            let lhs = self.clone().into_rx();
            let rhs = other.into_rx();
            Rx(
                crate::reactivity::DerivedPayload::new((lhs, rhs), |(lv, rv)| lv $op rv),
                ::core::marker::PhantomData,
            )
        }
    };
}

/// Provides a fluent API for checking equality on reactive values.
pub trait ReactivePartialEq: With + Clone + 'static
where
    Self::Value: PartialEq + Sized + 'static,
{
    reactive_compare_method!(equals, ==, PartialEq);
    reactive_compare_method!(not_equals, !=, PartialEq);
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: With + Clone + 'static
where
    Self::Value: PartialOrd + Sized + 'static,
{
    reactive_compare_method!(greater_than, >, PartialOrd);
    reactive_compare_method!(less_than, <, PartialOrd);
    reactive_compare_method!(greater_than_or_equals, >=, PartialOrd);
    reactive_compare_method!(less_than_or_equals, <=, PartialOrd);
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
                        $rx.rx_defined_at(),
                        $rx.rx_debug_name(),
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

#[doc(hidden)]
/// Provides a sensible panic message for accessing disposed signals.
#[macro_export]
macro_rules! unwrap_signal {
    ($signal:ident) => {{
        #[cfg(debug_assertions)]
        let location = std::panic::Location::caller();
        || {
            #[cfg(debug_assertions)]
            {
                panic!(
                    "{}",
                    $crate::traits::panic_getting_disposed_signal(
                        $signal.defined_at(),
                        $signal.debug_name(),
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

/// 自适应读取 Trait。
/// 用户无需关心底层是克隆还是借用，该 Trait 会根据类型自动选择最优路径。
pub trait Read: RxInternal {
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

    /// Clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get_untracked(&self) -> Option<Self::Value>
    where
        Self::Value: Sized + Clone,
    {
        self.try_read_untracked().map(|v| v.clone())
    }

    /// Clones and returns the value of the signal.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get_untracked(&self) -> Self::Value
    where
        Self::Value: Sized + Clone,
    {
        self.try_get_untracked()
            .unwrap_or_else(|| unwrap_rx!(self)())
    }

    /// Subscribes to the signal, then clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get(&self) -> Option<Self::Value>
    where
        Self::Value: Sized + Clone,
    {
        self.try_read().map(|v| v.clone())
    }

    /// Subscribes to the signal, then clones and returns the value of the signal.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get(&self) -> Self::Value
    where
        Self::Value: Sized + Clone,
    {
        self.try_get().unwrap_or_else(|| unwrap_rx!(self)())
    }
}

impl<T: ?Sized + RxInternal> Read for T {}

/// Allows disposing an arena-allocated signal before its owner has been disposed.
pub trait Dispose {
    /// Disposes of the signal. This:
    /// 1. Detaches the signal from the reactive graph, preventing it from triggering
    ///    further updates; and
    /// 2. Drops the value contained in the signal.
    fn dispose(self);
}

/// Allows tracking the value of some reactive data.
pub trait Track {
    /// Subscribes to this signal in the current reactive scope without doing anything with its value.
    #[track_caller]
    fn track(&self);
}

impl<T: ?Sized + RxInternal> Track for T {
    #[inline(always)]
    #[track_caller]
    fn track(&self) {
        self.rx_track();
    }
}

/// Give read-only access to a signal's value by reference inside a closure,
/// without tracking the value reactively.
pub trait WithUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value: ?Sized;

    /// Applies the closure to the value, and returns the result,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    /// Applies the closure to the value, and returns the result.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with_untracked(fun)
            .unwrap_or_else(unwrap_signal!(self))
    }
}

impl<T: ?Sized + RxInternal> WithUntracked for T {
    type Value = T::Value;
    #[inline(always)]
    #[track_caller]
    fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.rx_try_with_untracked(fun)
    }
}

/// Give read-only access to a signal's value by reference inside a closure,
/// and subscribes the active reactive observer (an effect or computed) to changes in its value.
pub trait With: DefinedAt {
    /// The type of the value contained in the signal.
    type Value: ?Sized;

    /// Subscribes to the signal, applies the closure to the value, and returns the result,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>;

    /// Subscribes to the signal, applies the closure to the value, and returns the result.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> U {
        self.try_with(fun).unwrap_or_else(unwrap_signal!(self))
    }
}

/// Allows creating a derived signal from this signal.
///
/// Unlike [`Get`], this trait uses [`WithUntracked`] as its basis, meaning it works
/// with the zero-copy closure-based access pattern.
pub trait Map: Sized {
    /// The type of the value contained in the signal.
    type Value: ?Sized;

    /// Creates a derived signal from this signal.
    fn map<U, F>(self, f: F) -> crate::Rx<DerivedPayload<Self, F>, crate::RxValue>
    where
        F: Fn(&Self::Value) -> U + Clone + 'static;
}

/// Allows converting a signal into a memoized signal.
///
/// Requires `Value: Clone + Sized` since memoization needs to clone and store values.
pub trait Memoize: With
where
    Self::Value: Clone + Sized,
{
    /// Creates a memoized signal from this signal.
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: PartialEq + 'static;
}

/// Notifies subscribers of a change in this signal.
pub trait Notify {
    /// Notifies subscribers of a change in this signal.
    #[track_caller]
    fn notify(&self);
}

/// Updates the value of a signal by applying a function that updates it in place,
/// without notifying subscribers.
pub trait UpdateUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value by applying a function, returning the value returned by that function.
    /// Does not notify subscribers that the signal has changed.
    ///
    /// # Panics
    /// Panics if you try to update a signal that has been disposed.
    #[track_caller]
    fn update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> U {
        self.try_update_untracked(fun)
            .unwrap_or_else(unwrap_signal!(self))
    }

    /// Updates the value by applying a function, returning the value returned by that function,
    /// or `None` if the signal has already been disposed.
    /// Does not notify subscribers that the signal has changed.
    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U>;
}

/// Updates the value of a signal by applying a function that updates it in place,
/// notifying its subscribers that the value has changed.
pub trait Update {
    /// The type of the value contained in the signal.
    type Value;

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
pub trait Set {
    /// The type of the value contained in the signal.
    type Value;

    /// Updates the value by replacing it, and notifies subscribers that it has changed.
    fn set(&self, value: Self::Value);

    /// Updates the value by replacing it, and notifies subscribers that it has changed.
    ///
    /// If the signal has already been disposed, returns `Some(value)` with the value that was
    /// passed in. Otherwise, returns `None`.
    fn try_set(&self, value: Self::Value) -> Option<Self::Value>;
}

/// Allows creating a setter closure from this signal.
pub trait SignalSetter: Sized {
    type Value;

    /// Creates a closure that sets the signal to the given value.
    fn setter(self, value: Self::Value) -> impl Fn() + Clone + 'static;
}

/// Allows creating an updater closure from this signal.
pub trait SignalUpdater: Sized {
    type Value;

    /// Creates a closure that updates the signal using the given function.
    fn updater<F>(self, f: F) -> impl Fn() + Clone + 'static
    where
        F: Fn(&mut Self::Value) + Clone + 'static;
}

/// Checks whether a signal has already been disposed.
pub trait IsDisposed {
    /// If `true`, the signal cannot be accessed without a panic.
    fn is_disposed(&self) -> bool;
}

impl<T: ?Sized + RxInternal> IsDisposed for T {
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.rx_is_disposed()
    }
}

/// Describes where the signal was defined. This is used for diagnostic warnings and is purely a
/// debug-mode tool.
pub trait DefinedAt {
    /// Returns the location at which the signal was defined. This is usually simply `None` in
    /// release mode.
    fn defined_at(&self) -> Option<&'static Location<'static>>;

    /// Returns the debug name of the signal, if any.
    fn debug_name(&self) -> Option<String> {
        None
    }
}

impl<T: ?Sized + RxInternal> DefinedAt for T {
    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        self.rx_defined_at()
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.rx_debug_name()
    }
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
pub trait SetUntracked: DefinedAt {
    /// The type of the value contained in the signal.
    type Value;

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
