//! 定义响应式原始组件（Signals, Memos, Rx 等）的行为核心。
//!
//! ## 设计哲学：零拷贝优先 & Rx 委托
//!
//! Silex 的 Trait 系统建立在以下两个核心理念之上：
//!
//! 1. **零拷贝 (Zero-Copy)**：[`With`]（基于闭包的访问）是系统的第一公民。
//!    - 信号值通常存储在 Arena（Map/Vec）中。访问它们涉及获取锁。
//!    - 使用闭包允许直接处理 `&T`，避免了不必要的 `Clone` 开销。
//!    - 天然支持动态大小类型（DST），如 `str` 或 `[T]`。
//! 2. **Rx 委托 (Rx Delegate)**：现代 Silex 采用委托模式。[`Rx`] 包装器是所有响应式操作（算术、比较、Map 等）的对外接口。
//!    - 通过 [`IntoRx`] 接口，常量、信号、闭包及元组都能无缝转化为统一的 `Rx`。
//!
//! ## 元组与组合性
//!
//! 虽然元组（如 `(Signal<A>, Signal<B>)`）现在通过 [`IntoRx`] 支持转换为 `Rx`，但需要注意：
//! - 这种转换实际上是“组合评估”。由于无法在物理内存连续获取 `&(A, B)`，它会克隆每个元素来构建结果元组。
//! - 对于追求极致性能、且需要多路访问的场景，请使用 [`batch_read!`] 宏实现真正的**零拷贝**。
//!
//! ## 核心原则
//! 1. **组合语义**：大多数高级 Trait（如 `With`, `Get`, `Map`）都是通过基础 Trait 组合而成的 Blanket Implementations。
//! 2. **原子化实现**：底层原语只需实现 [`Track`], [`Notify`], [`WithUntracked`] 和 [`UpdateUntracked`]。
//! 3. **容错性**：大多数读取操作包含 `try_` 变体，在信号已被销毁（Disposed）时安全返回 `None`。
//!
//! ## Trait 结构一览
//!
//! ### 基础核心 (底层实现者关注)
//! | Trait               | 作用                                                                           |
//! |---------------------|--------------------------------------------------------------------------------|
//! | [`Track`]           | 追踪变化，将当前值添加为当前响应式上下文的依赖。                               |
//! | [`Notify`]          | 通知订阅者该值已更改。                                                         |
//! | [`WithUntracked`]   | **核心原语**：提供对值的闭包式不可变访问（不追踪依赖）。                      |
//! | [`UpdateUntracked`] | 提供闭包式可变更新（不触发通知）。                                            |
//! | [`RxInternal`]      | **内部桥梁**：被包装在 `Rx` 内部的统一操作接口（对用户隐藏）。                |
//!
//! ### 派生能力 (基于自动实现)
//! | 类别 | Trait | 组合方式 | 描述 |
//! |------|-------|---------|------|
//! | **读取** | [`With`] | `WithUntracked` + `Track` | **核心 API**：带响应式追踪的闭包式访问。 |
//! | | [`Get`] | `With` + `Clone` | 复制当前值并追踪依赖（快捷方式）。 |
//! | | [`Map`] | `With` | 创建派生信号（`Derived`），返回 `Rx`。 |
//! | **更新** | [`Update`] | `UpdateUntracked` + `Notify` | 应用闭包并通知系统刷新。 |
//! | | [`Set`] | `Update` | 直接覆盖旧值。 |
//! | **转换** | [`IntoRx`] | — | **大一统接口**：将任意类型转化为统一的 `Rx`。 |
//!
//! ## 比较与算术运算
//!
//! 所有的 `Rx` 类型通过 [`ReactivePartialEq`] 和 [`ReactivePartialOrd`] 获得流畅的比较接口（如 `.equals()`），
//! 并自动支持标准算术运算符（`+`, `-`, `*`, `/` 等），这些运算都会返回一个新的包装过的 `Rx`。
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

use crate::reactivity::{DerivedPayload, Memo};
use crate::{Rx, RxValue};
use std::panic::Location;

#[doc(hidden)]
pub mod impls;

/// 允许将各种类型（原始类型、信号、Rx）转换为统一的 `Rx` 包装器。
///
/// *注意*: 原始类型（i32, f64, &str 等）会自动转换为 `Constant<T>`。
pub trait IntoRx {
    type Value;
    type RxType;
    fn into_rx(self) -> Self::RxType;
    fn is_constant(&self) -> bool;
}

/// A trait used internally by `Rx` to delegate calls to either a closure or a reactive primitive.
#[doc(hidden)]
pub trait RxInternal {
    type Value: ?Sized;
    fn rx_track(&self);
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U>;
    fn rx_defined_at(&self) -> Option<&'static std::panic::Location<'static>>;
    fn rx_debug_name(&self) -> Option<String>;
    fn rx_is_disposed(&self) -> bool;
    fn rx_is_constant(&self) -> bool;
}

pub type CompareFn<T> = fn(&T, &T) -> bool;

#[doc(hidden)]
macro_rules! reactive_compare_method {
    ($name:ident, $op:tt, $bound:ident) => {
        fn $name<O>(
            &self,
            other: O,
        ) -> Rx<
            DerivedPayload<
                (Self::RxType, O::RxType),
                fn(&<Self as IntoRx>::Value, &<Self as IntoRx>::Value) -> bool,
            >,
            RxValue,
        >
        where
            Self: IntoRx,
            Self::RxType: RxInternal<Value = <Self as IntoRx>::Value> + Clone + 'static,
            O: IntoRx<Value = <Self as IntoRx>::Value>,
            O::RxType: RxInternal<Value = <Self as IntoRx>::Value> + Clone + 'static,
            <Self as IntoRx>::Value: $bound + Clone + Sized + 'static,
        {
            let lhs = self.clone().into_rx();
            let rhs = other.into_rx();
            Rx(
                DerivedPayload::new((lhs, rhs), |lv, rv| lv $op rv),
                ::core::marker::PhantomData,
            )
        }
    };
}

/// Provides a fluent API for checking equality on reactive values.
pub trait ReactivePartialEq: With + Clone + 'static
where
    Self::Value: PartialEq + Clone + Sized + 'static,
{
    reactive_compare_method!(equals, ==, PartialEq);
    reactive_compare_method!(not_equals, !=, PartialEq);
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: With + Clone + 'static
where
    Self::Value: PartialOrd + Clone + Sized + 'static,
{
    reactive_compare_method!(greater_than, >, PartialOrd);
    reactive_compare_method!(less_than, <, PartialOrd);
    reactive_compare_method!(greater_than_or_equals, >=, PartialOrd);
    reactive_compare_method!(less_than_or_equals, <=, PartialOrd);
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

/// Extension trait: Clones the value of the signal, without tracking.
///
/// This is a **convenience trait** built on top of [`WithUntracked`]. It requires `T: Clone + Sized`.
/// For zero-copy access, prefer using [`WithUntracked::with_untracked`] directly.
///
/// # Performance Note
/// This trait performs a clone operation. On hot paths or with expensive-to-clone types,
/// prefer using [`WithUntracked::with_untracked`] instead.
pub trait GetUntracked: WithUntracked
where
    Self::Value: Clone + Sized,
{
    /// Clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get_untracked(&self) -> Option<Self::Value> {
        self.try_with_untracked(Clone::clone)
    }

    /// Clones and returns the value of the signal.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get_untracked(&self) -> Self::Value {
        self.try_get_untracked()
            .unwrap_or_else(unwrap_signal!(self))
    }
}

/// Extension trait: Clones the value of the signal, with reactive tracking.
///
/// This is a **convenience trait** built on top of [`With`]. It requires `T: Clone + Sized`.
/// For zero-copy access, prefer using [`With::with`] directly.
///
/// # Performance Note
/// This trait performs a clone operation. On hot paths or with expensive-to-clone types,
/// prefer using [`With::with`] instead.
pub trait Get: With
where
    Self::Value: Clone + Sized,
{
    /// Subscribes to the signal, then clones and returns the value of the signal,
    /// or `None` if the signal has already been disposed.
    #[track_caller]
    fn try_get(&self) -> Option<Self::Value> {
        self.try_with(Clone::clone)
    }

    /// Subscribes to the signal, then clones and returns the value of the signal.
    ///
    /// # Panics
    /// Panics if you try to access a signal that has been disposed.
    #[track_caller]
    fn get(&self) -> Self::Value {
        self.try_get().unwrap_or_else(unwrap_signal!(self))
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
