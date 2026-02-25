use crate::reactivity::{Constant, NodeId};
use crate::traits::guards::*;
use crate::traits::{RxBase, RxValue};
use crate::{Rx, RxValueKind};
use std::ops::Deref;
use std::panic::Location;

/// 允许将各种类型（原始类型、信号、Rx）转换为统一的 `Rx` 包装器。
///
/// *注意*: 原始类型（i32, f64, &str 等）会自动转换为 `Constant<T>`。
pub trait IntoRx: RxValue {
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
}

/// 克隆获取特质。仅当值支持克隆时自动生效。
pub trait RxGet: RxRead
where
    Self::Value: Clone + Sized,
    for<'a> Self::ReadOutput<'a>: Deref<Target = Self::Value>,
{
    // ==========================================
    // 3. 克隆获取（Getters）
    // ==========================================

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

    /// 非响应式地克隆和返回值。如果是被销毁的，返回 None。
    #[track_caller]
    fn try_get_untracked(&self) -> Option<Self::Value> {
        self.try_read_untracked().map(|v| (*v).clone())
    }

    /// 非响应式地克隆和返回值。
    ///
    /// # Panics
    /// 访问被销毁的信号时报错。
    #[track_caller]
    fn get_untracked(&self) -> Self::Value {
        self.try_get_untracked()
            .unwrap_or_else(|| unwrap_rx!(self)())
    }

    /// 响应式地订阅信号，克隆并返回值。已被销毁则返回 None。
    #[track_caller]
    fn try_get(&self) -> Option<Self::Value> {
        self.try_read().map(|v| (*v).clone())
    }

    /// 响应式地订阅信号，克隆并返回值。
    ///
    /// # Panics
    /// 访问被销毁的信号时报错。
    #[track_caller]
    fn get(&self) -> Self::Value {
        self.try_get().unwrap_or_else(|| unwrap_rx!(self)())
    }
}

impl<T: ?Sized + RxRead> RxGet for T
where
    T::Value: Clone + Sized,
    for<'a> T::ReadOutput<'a>: Deref<Target = T::Value>,
{
}

impl<T: ?Sized + RxInternal> RxRead for T where for<'a> T::ReadOutput<'a>: Deref<Target = T::Value> {}

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

// --- Implementations moved from impls.rs ---

macro_rules! impl_closure_rx {
    (
        impl<$($gen:ident),*> $target:ty $(where $($bounds:tt)*)?
    ) => {
        impl<$($gen),*> $crate::traits::RxValue for $target $(where $($bounds)*, T: 'static)? {
            type Value = T;
        }

        impl<$($gen),*> RxBase for $target $(where $($bounds)*, T: 'static)? {
            #[inline(always)] fn id(&self) -> Option<NodeId> { None }
            #[inline(always)] fn track(&self) {}
            #[inline(always)] fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> { None }
        }

        impl<$($gen),*> RxInternal for $target $(where $($bounds)*, T: 'static)? {
            type ReadOutput<'a> = RxGuard<'a, T, T> where Self: 'a;

            #[inline(always)] fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> { let val = (self)(); Some(RxGuard::Owned(val)) }
            #[inline(always)] fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> { self.rx_read_untracked().map(|g| fun(&*g)) }
            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                false
            }
        }

    };
}

impl_closure_rx!(impl<F, T> F where F: Fn() -> T);
impl_closure_rx!(impl<T> ::std::rc::Rc<dyn Fn() -> T>);

impl<F: crate::traits::RxValue, M> crate::traits::RxValue for Rx<F, M> {
    type Value = F::Value;
}

impl<F: RxBase, M> RxBase for Rx<F, M> {
    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        self.0.id()
    }

    #[inline(always)]
    fn track(&self) {
        self.0.track();
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.0.is_disposed()
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
        self.0.defined_at()
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        self.0.debug_name()
    }
}

impl<F: RxInternal, M> RxInternal for Rx<F, M> {
    type ReadOutput<'a>
        = F::ReadOutput<'a>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read(&self) -> Option<Self::ReadOutput<'_>> {
        self.0.rx_read()
    }
    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        self.0.rx_read_untracked()
    }
    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.0.rx_try_with_untracked(fun)
    }
    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.0.rx_get_adaptive()
    }
    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        self.0.rx_is_constant()
    }
}

// --- 元组 RxInternal 实现：支持递归常量检测 ---

macro_rules! impl_tuple_into_rx {
    ($len:expr, $($T:ident : $idx:tt),+) => {
        #[allow(non_snake_case)]
        impl<$($T),+> $crate::traits::RxValue for ($($T,)+)
        where
            $($T: $crate::traits::RxValue),+,
            $($T::Value: core::marker::Sized),+
        {
            type Value = ($($T::Value,)+);
        }

        #[allow(non_snake_case)]
        impl<$($T),+> IntoRx for ($($T,)+)
        where
            $($T: IntoRx + Clone + 'static),+,
            $($T::Value: Clone + 'static),+
        {
            type RxType = Rx<crate::reactivity::OpPayload<Self::Value, $len>, RxValueKind>;

            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                let is_constant = $(self.$idx.is_constant() && )+ true;
                let signals = ($(self.$idx.into_signal(),)+);

                #[inline(always)]
                unsafe fn read_impl<$($T: Clone + 'static),+>(inputs: &[crate::reactivity::NodeId]) -> Option<($($T,)+)> {
                    unsafe {
                        Some(($(
                            crate::reactivity::rx_borrow_signal_unsafe::<$T>(inputs[$idx])?.clone(),
                        )+))
                    }
                }

                let inputs = [$(signals.$idx.ensure_node_id()),+];

                Rx(crate::reactivity::OpPayload {
                    inputs,
                    read: read_impl::<$($T::Value),+>,
                    track: crate::reactivity::op_trampolines::track_inputs,
                    is_constant,
                }, ::core::marker::PhantomData)
            }

            #[inline(always)]
            fn is_constant(&self) -> bool { $(self.$idx.is_constant() && )+ true }

            #[inline(always)]
            fn into_signal(self) -> crate::reactivity::Signal<Self::Value>
            where
                Self: 'static,
            {
                let s = self.clone();
                crate::reactivity::Signal::derive(move || s.clone().into_rx().get())
            }
        }

        impl<$($T),+> RxBase for ($($T,)+)
        where
            $($T: RxBase),+,
            $($T::Value: Sized),+
        {
            #[inline(always)] fn id(&self) -> Option<crate::reactivity::NodeId> { None }
            #[inline(always)] fn track(&self) { $(self.$idx.track();)+ }
            #[inline(always)] fn is_disposed(&self) -> bool { $(self.$idx.is_disposed() || )+ false }
            #[inline(always)] fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> { None }
            #[inline(always)] fn debug_name(&self) -> Option<String> { None }
        }

        impl<$($T),+> RxInternal for ($($T,)+)
        where
            $($T: RxInternal + Clone + 'static),+,
            $($T: IntoRx),+,
            $($T::Value: Clone + Sized + 'static),+
        {
            type ReadOutput<'a> = RxGuard<'a, Self::Value, Self::Value> where Self: 'a;

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                Some(RxGuard::Owned(self.rx_get_adaptive()?))
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                self.rx_get_adaptive().map(|v| fun(&v))
            }

            #[inline(always)]
            fn rx_get_adaptive(&self) -> Option<Self::Value>
            where
                Self::Value: Sized,
            {
                Some(($(
                    self.$idx.rx_get_adaptive()?,
                )+))
            }

            #[inline(always)] fn rx_is_constant(&self) -> bool { $(self.$idx.rx_is_constant() && )+ true }
        }
    };
}

impl_tuple_into_rx!(2, T0: 0, T1: 1);
impl_tuple_into_rx!(3, T0: 0, T1: 1, T2: 2);
impl_tuple_into_rx!(4, T0: 0, T1: 1, T2: 2, T3: 3);
impl_tuple_into_rx!(5, T0: 0, T1: 1, T2: 2, T3: 3, T4: 4);
impl_tuple_into_rx!(6, T0: 0, T1: 1, T2: 2, T3: 3, T4: 4, T5: 5);

impl<F, M> IntoRx for Rx<F, M>
where
    F: RxInternal + Clone + 'static,
    F::Value: Clone + 'static,
    for<'a> F::ReadOutput<'a>: std::ops::Deref<Target = F::Value>,
{
    type RxType = Self;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        self
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.0.rx_is_constant()
    }

    #[inline(always)]
    fn into_signal(self) -> crate::reactivity::Signal<Self::Value>
    where
        Self: 'static,
    {
        crate::reactivity::Signal::derive(move || self.get())
    }
}

macro_rules! impl_into_rx_primitive {
    ($($t:ty $(: $val:ty => $conv:expr)?),*) => {
        $(
            impl $crate::traits::RxValue for $t {
                type Value = impl_into_rx_primitive!(@type $t $(, $val)?);
            }

            impl IntoRx for $t {
                type RxType = Rx<Constant<Self::Value>, RxValueKind>;

                #[inline(always)]
                fn into_rx(self) -> Self::RxType {
                    let val = impl_into_rx_primitive!(@val self $(, $conv)?);
                    Rx(Constant(val), ::core::marker::PhantomData)
                }

                #[inline(always)]
                fn is_constant(&self) -> bool {
                    true
                }

                #[inline(always)]
                fn into_signal(self) -> crate::reactivity::Signal<Self::Value> {
                    crate::reactivity::Signal::from(impl_into_rx_primitive!(@val self $(, $conv)?))
                }
            }
        )*
    };
    (@type $t:ty) => { $t };
    (@type $t:ty, $val:ty) => { $val };
    (@val $self:ident) => { $self };
    (@val $self:ident, $conv:expr) => { ($conv)($self) };
}

impl_into_rx_primitive!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64,
    String,
    &str : String => |s: &str| s.to_string()
);

#[macro_export]
macro_rules! impl_rx_delegate {
    ($target:ident, $is_const:expr) => {
        impl<T: 'static> $crate::traits::RxValue for $target<T> {
            type Value = T;
        }

        impl<T: Clone + 'static> $crate::traits::RxBase for $target<T> {
            #[inline(always)]
            fn id(&self) -> Option<$crate::reactivity::NodeId> {
                None
            }
            #[inline(always)]
            fn track(&self) {}
            #[inline(always)]
            fn is_disposed(&self) -> bool {
                false
            }
            #[inline(always)]
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                None
            }
            #[inline(always)]
            fn debug_name(&self) -> Option<String> {
                None
            }
        }

        impl<T: Clone + 'static> $crate::traits::IntoRx for $target<T> {
            type RxType = $crate::Rx<Self, $crate::RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                $crate::Rx(self, ::core::marker::PhantomData)
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                $is_const
            }
            #[inline(always)]
            fn into_signal(self) -> $crate::reactivity::Signal<T> {
                $crate::reactivity::Signal::derive(move || $crate::traits::RxRead::get(&self))
            }
        }
    };
    ($target:ident, SignalID, $is_const:expr) => {
        impl<T: 'static> $crate::traits::RxValue for $target<T> {
            type Value = T;
        }

        impl<T: 'static> $crate::traits::RxBase for $target<T> {
            #[inline(always)]
            fn id(&self) -> Option<$crate::reactivity::NodeId> {
                Some(self.id)
            }
            #[inline(always)]
            fn track(&self) {
                ::silex_reactivity::track_signal(self.id);
            }
            #[inline(always)]
            fn is_disposed(&self) -> bool {
                !::silex_reactivity::is_signal_valid(self.id)
            }
            #[inline(always)]
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                ::silex_reactivity::get_node_defined_at(self.id)
            }
            #[inline(always)]
            fn debug_name(&self) -> Option<String> {
                ::silex_reactivity::get_debug_label(self.id)
            }
        }

        impl<T: Clone + 'static> $crate::traits::IntoRx for $target<T> {
            type RxType = $crate::Rx<Self, $crate::RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                $crate::Rx(self, ::core::marker::PhantomData)
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                $is_const
            }
            #[inline(always)]
            fn into_signal(self) -> $crate::reactivity::Signal<T> {
                $crate::reactivity::Signal::Read($crate::reactivity::ReadSignal {
                    id: self.id,
                    marker: ::core::marker::PhantomData,
                })
            }
        }

        impl<T: 'static> $crate::traits::RxInternal for $target<T> {
            type ReadOutput<'a>
                = $crate::traits::RxGuard<'a, T, T>
            where
                Self: 'a;

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                let id = self.id;
                unsafe {
                    ::silex_reactivity::try_with_signal_untracked(id, |v: &T| {
                        std::mem::transmute::<&T, &'static T>(v)
                    })
                    .map(|v| $crate::traits::RxGuard::Borrowed {
                        value: v,
                        token: Some($crate::NodeRef::from_id(id)),
                    })
                }
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                ::silex_reactivity::try_with_signal_untracked(self.id, fun)
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

            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                $is_const
            }
        }
    };
    ($target:ident, $field:ident, $is_const:expr) => {
        impl<T: 'static> $crate::traits::RxValue for $target<T> {
            type Value = T;
        }

        impl<T: 'static> $crate::traits::RxBase for $target<T> {
            #[inline(always)]
            fn id(&self) -> Option<$crate::reactivity::NodeId> {
                $crate::traits::RxBase::id(&self.$field)
            }
            #[inline(always)]
            fn track(&self) {
                $crate::traits::RxBase::track(&self.$field)
            }
            #[inline(always)]
            fn is_disposed(&self) -> bool {
                $crate::traits::RxBase::is_disposed(&self.$field)
            }
            #[inline(always)]
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                $crate::traits::RxBase::defined_at(&self.$field)
            }
            #[inline(always)]
            fn debug_name(&self) -> Option<String> {
                $crate::traits::RxBase::debug_name(&self.$field)
            }
        }

        impl<T: Clone + 'static> $crate::traits::IntoRx for $target<T> {
            type RxType = $crate::Rx<Self, $crate::RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                $crate::Rx(self, ::core::marker::PhantomData)
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                $is_const
            }
            #[inline(always)]
            fn into_signal(self) -> $crate::reactivity::Signal<T> {
                $crate::traits::IntoRx::into_signal(self.$field)
            }
        }

        impl<T: 'static> $crate::traits::RxInternal for $target<T> {
            type ReadOutput<'a>
                = <$crate::reactivity::ReadSignal<T> as $crate::traits::RxInternal>::ReadOutput<'a>
            where
                Self: 'a;

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                self.$field.rx_read_untracked()
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                self.$field.rx_try_with_untracked(fun)
            }

            #[inline(always)]
            fn rx_get_adaptive(&self) -> Option<Self::Value>
            where
                Self::Value: Sized,
            {
                self.$field.rx_get_adaptive()
            }

            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                $is_const
            }
        }
    };
}
