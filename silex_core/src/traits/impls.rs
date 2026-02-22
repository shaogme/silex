use super::*;
use crate::reactivity::{Constant, DerivedPayload, Memo};
use crate::{Rx, RxValue};
use std::panic::Location;

/// A module containing static helper functions for reactive operations.
/// usage of these avoids generating unique closures for every operator implementation.
#[doc(hidden)]
pub mod ops_impl {
    use std::ops::*;
    macro_rules! gen_ops {
        (bin: $($fn:ident:$trait:ident),*; un: $($ufn:ident:$utrait:ident),*) => {
            $(
                #[inline]
                pub fn $fn<T: $trait<Output = T> + Clone>(lhs: &T, rhs: &T) -> T {
                    lhs.clone().$fn(rhs.clone())
                }
            )*
            $(
                #[inline]
                pub fn $ufn<T: $utrait<Output = T> + Clone>(val: &T) -> T {
                    val.clone().$ufn()
                }
            )*
        };
    }
    gen_ops!(
        bin: add:Add, sub:Sub, mul:Mul, div:Div, rem:Rem, bitand:BitAnd, bitor:BitOr, bitxor:BitXor, shl:Shl, shr:Shr;
        un: neg:Neg, not:Not
    );
}

impl<F, T> RxInternal for F
where
    F: Fn() -> T,
{
    type Value = T;

    #[inline(always)]
    fn rx_track(&self) {}

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        let val = (self)();
        Some(fun(&val))
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
}

impl<T> RxInternal for ::std::rc::Rc<dyn Fn() -> T> {
    type Value = T;

    #[inline(always)]
    fn rx_track(&self) {}

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        let val = (self)();
        Some(fun(&val))
    }

    #[inline(always)]
    fn rx_defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
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
}

macro_rules! impl_rx_wrapper_traits {
    () => {
        impl<F, M> RxInternal for Rx<F, M>
        where
            F: RxInternal,
        {
            type Value = F::Value;

            #[inline(always)]
            fn rx_track(&self) {
                self.0.rx_track();
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                self.0.rx_try_with_untracked(fun)
            }

            #[inline(always)]
            fn rx_defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                self.0.rx_defined_at()
            }

            #[inline(always)]
            fn rx_debug_name(&self) -> Option<String> {
                self.0.rx_debug_name()
            }

            #[inline(always)]
            fn rx_is_disposed(&self) -> bool {
                self.0.rx_is_disposed()
            }

            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                self.0.rx_is_constant()
            }
        }

        impl<F, M> DefinedAt for Rx<F, M>
        where
            F: RxInternal,
        {
            #[inline(always)]
            fn defined_at(&self) -> Option<&'static Location<'static>> {
                self.0.rx_defined_at()
            }
            #[inline(always)]
            fn debug_name(&self) -> Option<String> {
                self.0.rx_debug_name()
            }
        }

        impl<F, M> IsDisposed for Rx<F, M>
        where
            F: RxInternal,
        {
            #[inline(always)]
            fn is_disposed(&self) -> bool {
                self.0.rx_is_disposed()
            }
        }

        impl<F, M> Track for Rx<F, M>
        where
            F: RxInternal,
        {
            #[inline(always)]
            fn track(&self) {
                self.0.rx_track();
            }
        }

        impl<F, M> WithUntracked for Rx<F, M>
        where
            F: RxInternal,
        {
            type Value = F::Value;

            #[inline(always)]
            fn try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                self.0.rx_try_with_untracked(fun)
            }
        }
    };
}

impl_rx_wrapper_traits!();

// --- 元组 RxInternal 实现：支持递归常量检测 ---

#[macro_export]
macro_rules! impl_rx_internal_tuple_helper {
    (@call $fun:ident, $self:ident, [$($acc:expr),*] [$head:tt]) => {
        $self.$head.rx_try_with_untracked(|v| {
            let val = ($($acc.clone(),)* v.clone());
            $fun(&val)
        })
    };
    (@call $fun:ident, $self:ident, [$($acc:expr),*] [$head:tt, $($tail:tt),+]) => {
        $self.$head.rx_try_with_untracked(|v| {
            $crate::impl_rx_internal_tuple_helper!(@call $fun, $self, [$($acc,)* v] [$($tail),+])
        }).flatten()
    };
}

macro_rules! impl_tuple_everything {
    ($($T:ident : $idx:tt),+) => {
        impl_tuple_everything!(@impl $($T : $idx),+);
    };
    ($($T:ident : $idx:tt),+ ; into) => {
        impl_tuple_everything!(@impl $($T : $idx),+);
        #[allow(non_snake_case)]
        impl<$($T),+> IntoRx for ($($T,)+)
        where
            $($T: IntoRx),+,
            $($T::RxType: RxInternal<Value = $T::Value> + Clone + 'static),+,
            $($T::Value: Clone + 'static),+
        {
            type Value = ($($T::Value,)+);
            type RxType = Rx<DerivedPayload<($($T::RxType,)+), fn($(&$T::Value,)+) -> ($($T::Value,)+)>, RxValue>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                let ($($T,)+) = self;
                $(let $T = $T.into_rx();)+
                Rx(DerivedPayload::new(($($T,)+), |$($T),+| ($($T.clone(),)+)), ::core::marker::PhantomData)
            }
            #[inline(always)]
            fn is_constant(&self) -> bool { $(self.$idx.is_constant() && )+ true }
        }
    };
    (@impl $($T:ident : $idx:tt),+) => {
        impl<$($T),+> RxInternal for ($($T,)+)
        where
            $($T: RxInternal),+,
            $($T::Value: Clone + Sized),+
        {
            type Value = ($($T::Value,)+);
            #[inline(always)] fn rx_track(&self) { $(self.$idx.rx_track();)+ }
            #[inline(always)] fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                $crate::impl_rx_internal_tuple_helper!(@call fun, self, [] [$($idx),+])
            }
            #[inline(always)] fn rx_defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> { None }
            #[inline(always)] fn rx_debug_name(&self) -> Option<String> { None }
            #[inline(always)] fn rx_is_disposed(&self) -> bool { $(self.$idx.rx_is_disposed() || )+ false }
            #[inline(always)] fn rx_is_constant(&self) -> bool { $(self.$idx.rx_is_constant() && )+ true }
        }
        impl<$($T: Track),+> Track for ($($T,)+) { #[inline(always)] fn track(&self) { $(self.$idx.track();)+ } }
        impl<$($T: IsDisposed),+> IsDisposed for ($($T,)+) { #[inline(always)] fn is_disposed(&self) -> bool { $(self.$idx.is_disposed() || )+ false } }
        impl<$($T: DefinedAt),+> DefinedAt for ($($T,)+) { #[inline(always)] fn defined_at(&self) -> Option<&'static std::panic::Location<'static>> { None } }
    };
}

impl_tuple_everything!(T0: 0, T1: 1; into);
impl_tuple_everything!(T0: 0, T1: 1, T2: 2; into);
impl_tuple_everything!(T0: 0, T1: 1, T2: 2, T3: 3; into);
impl_tuple_everything!(T0: 0, T1: 1, T2: 2, T3: 3, T4: 4);
impl_tuple_everything!(T0: 0, T1: 1, T2: 2, T3: 3, T4: 4, T5: 5);

impl<F, M> IntoRx for Rx<F, M>
where
    F: RxInternal,
    F::Value: Sized,
{
    type Value = F::Value;
    type RxType = Self;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        self
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        self.0.rx_is_constant()
    }
}

macro_rules! impl_into_rx_primitive {
    ($($t:ty $(: $val:ty => $conv:expr)?),*) => {
        $(
            impl IntoRx for $t {
                type Value = impl_into_rx_primitive!(@type $t $(, $val)?);
                type RxType = Rx<Constant<Self::Value>, RxValue>;

                #[inline(always)]
                fn into_rx(self) -> Self::RxType {
                    let val = impl_into_rx_primitive!(@val self $(, $conv)?);
                    Rx(Constant(val), ::core::marker::PhantomData)
                }

                #[inline(always)]
                fn is_constant(&self) -> bool {
                    true
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
        // 1. 实现 IntoRx 接口
        impl<T: Clone + 'static> $crate::traits::IntoRx for $target<T> {
            type Value = T;
            type RxType = $crate::Rx<Self, $crate::RxValue>; // 直接塞入目标本身

            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                $crate::Rx(self, ::core::marker::PhantomData)
            }

            #[inline(always)]
            fn is_constant(&self) -> bool {
                $is_const
            }
        }

        // 2. 实现 RxInternal (核心改动)
        impl<T: 'static> $crate::traits::RxInternal for $target<T> {
            type Value = <$target<T> as $crate::traits::WithUntracked>::Value;

            #[inline(always)]
            fn rx_track(&self) {
                $crate::traits::Track::track(self);
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                self.try_with_untracked(fun)
            }

            #[inline(always)]
            fn rx_defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                $crate::traits::DefinedAt::defined_at(self)
            }

            #[inline(always)]
            fn rx_debug_name(&self) -> Option<String> {
                $crate::traits::DefinedAt::debug_name(self)
            }

            #[inline(always)]
            fn rx_is_disposed(&self) -> bool {
                $crate::traits::IsDisposed::is_disposed(self)
            }

            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                $is_const
            }
        }
    };
}
#[macro_export]
macro_rules! impl_reactive_ops {
    ($target:ident) => {
        $crate::impl_reactive_op!($target, Add, add);
        $crate::impl_reactive_op!($target, Sub, sub);
        $crate::impl_reactive_op!($target, Mul, mul);
        $crate::impl_reactive_op!($target, Div, div);
        $crate::impl_reactive_op!($target, Rem, rem);
        $crate::impl_reactive_op!($target, BitAnd, bitand);
        $crate::impl_reactive_op!($target, BitOr, bitor);
        $crate::impl_reactive_op!($target, BitXor, bitxor);
        $crate::impl_reactive_op!($target, Shl, shl);
        $crate::impl_reactive_op!($target, Shr, shr);

        $crate::impl_reactive_unary_op!($target, Neg, neg);
        $crate::impl_reactive_unary_op!($target, Not, not);
    };
}

#[macro_export]
macro_rules! impl_rx_ops {
    () => {
        $crate::impl_rx_op!(Add, add);
        $crate::impl_rx_op!(Sub, sub);
        $crate::impl_rx_op!(Mul, mul);
        $crate::impl_rx_op!(Div, div);
        $crate::impl_rx_op!(Rem, rem);
        $crate::impl_rx_op!(BitAnd, bitand);
        $crate::impl_rx_op!(BitOr, bitor);
        $crate::impl_rx_op!(BitXor, bitxor);
        $crate::impl_rx_op!(Shl, shl);
        $crate::impl_rx_op!(Shr, shr);

        $crate::impl_rx_unary_op!(Neg, neg);
        $crate::impl_rx_unary_op!(Not, not);
    };
}

#[macro_export]
macro_rules! impl_rx_op {
    ($trait:ident, $method:ident) => {
        impl<F, R, T> std::ops::$trait<R> for $crate::Rx<F, $crate::RxValue>
        where
            F: $crate::traits::RxInternal<Value = T> + Clone + 'static,
            T: std::ops::$trait<T, Output = T> + Clone + 'static,
            R: $crate::traits::IntoRx,
            R::RxType: $crate::traits::RxInternal<Value = T> + Clone + 'static,
        {
            type Output = $crate::Rx<
                $crate::reactivity::DerivedPayload<(Self, R::RxType), fn(&T, &T) -> T>,
                $crate::RxValue,
            >;

            #[inline]
            fn $method(self, rhs: R) -> Self::Output {
                let lhs = self;
                let rhs = rhs.into_rx();
                $crate::Rx(
                    $crate::reactivity::DerivedPayload::new(
                        (lhs, rhs),
                        $crate::traits::impls::ops_impl::$method,
                    ),
                    ::core::marker::PhantomData,
                )
            }
        }
    };
}

#[macro_export]
macro_rules! impl_rx_unary_op {
    ($trait:ident, $method:ident) => {
        impl<F, T> std::ops::$trait for $crate::Rx<F, $crate::RxValue>
        where
            F: $crate::traits::RxInternal<Value = T> + Clone + 'static,
            T: std::ops::$trait<Output = T> + Clone + 'static,
        {
            type Output =
                $crate::Rx<$crate::reactivity::DerivedPayload<Self, fn(&T) -> T>, $crate::RxValue>;

            #[inline]
            fn $method(self) -> Self::Output {
                $crate::Rx(
                    $crate::reactivity::DerivedPayload::new(
                        self,
                        $crate::traits::impls::ops_impl::$method,
                    ),
                    ::core::marker::PhantomData,
                )
            }
        }
    };
}

#[macro_export]
macro_rules! impl_reactive_op {
    ($target:ident, $trait:ident, $method:ident) => {
        impl<T, R> std::ops::$trait<R> for $target<T>
        where
            $target<T>: $crate::traits::IntoRx,
            <$target<T> as $crate::traits::IntoRx>::RxType: std::ops::$trait<R>,
        {
            type Output =
                <<$target<T> as $crate::traits::IntoRx>::RxType as std::ops::$trait<R>>::Output;

            #[inline(always)]
            fn $method(self, rhs: R) -> Self::Output {
                $crate::traits::IntoRx::into_rx(self).$method(rhs)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_reactive_unary_op {
    ($target:ident, $trait:ident, $method:ident) => {
        impl<T> std::ops::$trait for $target<T>
        where
            $target<T>: $crate::traits::IntoRx,
            <$target<T> as $crate::traits::IntoRx>::RxType: std::ops::$trait,
        {
            type Output =
                <<$target<T> as $crate::traits::IntoRx>::RxType as std::ops::$trait>::Output;

            #[inline(always)]
            fn $method(self) -> Self::Output {
                $crate::traits::IntoRx::into_rx(self).$method()
            }
        }
    };
}

crate::impl_rx_ops!();

impl<S> ReactivePartialEq for S
where
    S: With + Clone + 'static,
    <S as With>::Value: PartialEq + Clone + Sized + 'static,
{
}

impl<S> ReactivePartialOrd for S
where
    S: With + Clone + 'static,
    <S as With>::Value: PartialOrd + Clone + Sized + 'static,
{
}

impl<T> With for T
where
    T: WithUntracked + Track,
{
    type Value = <T as WithUntracked>::Value;

    #[track_caller]
    fn try_with<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        self.track();
        self.try_with_untracked(fun)
    }
}

// Blanket implementation: any type with WithUntracked where Value: Clone + Sized gets GetUntracked
impl<T> GetUntracked for T
where
    T: WithUntracked,
    T::Value: Clone + Sized,
{
}

// Blanket implementation: any type with With where Value: Clone + Sized gets Get
impl<T> Get for T
where
    T: With,
    T::Value: Clone + Sized,
{
}

// Map is based on WithUntracked, not Get - this is intentional for zero-copy support
impl<S> Map for S
where
    S: WithUntracked + Track + Clone + 'static,
{
    type Value = S::Value;

    fn map<U, F>(self, f: F) -> crate::Rx<DerivedPayload<Self, F>, crate::RxValue>
    where
        F: Fn(&Self::Value) -> U + Clone + 'static,
    {
        crate::Rx(DerivedPayload::new(self, f), ::core::marker::PhantomData)
    }
}

impl<T> Memoize for T
where
    T: With + Clone + 'static,
    T::Value: Clone + Sized,
{
    fn memo(self) -> Memo<Self::Value>
    where
        Self::Value: PartialEq + 'static,
    {
        let this = self.clone();
        Memo::new(move |_| this.with(Clone::clone))
    }
}

impl<T> Set for T
where
    T: Update + IsDisposed,
{
    type Value = <Self as Update>::Value;

    #[track_caller]
    fn set(&self, value: Self::Value) {
        self.try_update(|n| *n = value);
    }

    #[track_caller]
    fn try_set(&self, value: Self::Value) -> Option<Self::Value> {
        if self.is_disposed() {
            Some(value)
        } else {
            self.set(value);
            None
        }
    }
}

impl<T> SetUntracked for T
where
    T: UpdateUntracked + IsDisposed,
{
    type Value = <Self as UpdateUntracked>::Value;

    #[track_caller]
    fn try_set_untracked(&self, value: Self::Value) -> Option<Self::Value> {
        if self.is_disposed() {
            Some(value)
        } else {
            self.update_untracked(|n| *n = value);
            None
        }
    }
}
