use super::*;
use crate::reactivity::{Constant, DerivedPayload, Memo};
use crate::{Rx, RxValue};
use std::cell::OnceCell;
use std::marker::PhantomData;

/// A module containing static helper functions for reactive operations.
/// usage of these avoids generating unique closures for every operator implementation.
#[doc(hidden)]
pub mod ops_impl {
    use std::ops::*;
    macro_rules! gen_ops {
        (bin: $($fn:ident:$trait:ident),*; un: $($ufn:ident:$utrait:ident),*) => {
            $(
                #[inline]
                pub fn $fn<T>(lhs: &T, rhs: &T) -> T
                where
                    for<'a> &'a T: $trait<&'a T, Output = T>,
                {
                    lhs.$fn(rhs)
                }
            )*
            $(
                #[inline]
                pub fn $ufn<T>(val: &T) -> T
                where
                    for<'a> &'a T: $utrait<Output = T>,
                {
                    val.$ufn()
                }
            )*
        };
    }
    gen_ops!(
        bin: add:Add, sub:Sub, mul:Mul, div:Div, rem:Rem, bitand:BitAnd, bitor:BitOr, bitxor:BitXor, shl:Shl, shr:Shr;
        un: neg:Neg, not:Not
    );

    macro_rules! gen_cmp_ops {
        ($($fn:ident:$op:tt:$bound:ident),*) => {
            $(
                #[inline]
                pub fn $fn<T>(lhs: &T, rhs: &T) -> bool
                where
                    T: $bound,
                {
                    lhs $op rhs
                }
            )*
        };
    }
    gen_cmp_ops!(
        eq:==:PartialEq, ne:!=:PartialEq,
        gt:>:PartialOrd, lt:<:PartialOrd, ge:>=:PartialOrd, le:<=:PartialOrd
    );
}

macro_rules! impl_closure_rx {
    (
        impl<$($gen:ident),*> $target:ty $(where $($bounds:tt)*)?
    ) => {
        impl<$($gen),*> RxInternal for $target $(where $($bounds)*, T: 'static)? {
            type Value = T;
            type ReadOutput<'a> = RxGuard<'a, T, T> where Self: 'a;

            #[inline(always)] fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> { let val = (self)(); Some(RxGuard::Owned(val)) }
        }
    };
}

impl_closure_rx!(impl<F, T> F where F: Fn() -> T);
impl_closure_rx!(impl<T> ::std::rc::Rc<dyn Fn() -> T>);

impl<F: RxInternal, M> RxInternal for Rx<F, M> {
    type Value = F::Value;
    type ReadOutput<'a>
        = F::ReadOutput<'a>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_track(&self) {
        self.0.rx_track();
    }
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
    fn rx_defined_at(&self) -> Option<&'static std::panic::Location<'static>> {
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

// --- 元组 RxInternal 实现：支持递归常量检测 ---

// 已移除旧的递归助手，改用更高效的直接构造方案

macro_rules! impl_tuple_everything {
    ($guard:ident, $($T:ident : $idx:tt),+) => {
        impl_tuple_everything!(@impl $guard, $($T : $idx),+);
    };
    ($guard:ident, $($T:ident : $idx:tt),+ ; into) => {
        impl_tuple_everything!(@impl $guard, $($T : $idx),+);
        #[allow(non_snake_case)]
        impl<$($T),+> IntoRx for ($($T,)+)
        where
            $($T: IntoRx + Clone + 'static),+,
            $($T::RxType: RxInternal<Value = $T::Value> + Clone + 'static),+,
            $($T::Value: Clone + 'static),+
        {
            type Value = ($($T::Value,)+);
            type RxType = Rx<
                DerivedPayload<
                    ($(crate::reactivity::Signal<$T::Value>,)+),
                    fn(&($($T::Value,)+)) -> ($($T::Value,)+)
                >,
                RxValue
            >;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                let ($($T,)+) = self;
                $(let $T = $T.into_signal();)+
                Rx(DerivedPayload::new(($($T,)+), |t| ($(t.$idx.clone(),)+)), ::core::marker::PhantomData)
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
    };
    (@impl $guard:ident, $($T:ident : $idx:tt),+) => {
        impl<$($T),+> RxInternal for ($($T,)+)
        where
            $($T: RxInternal),+,
            $($T::Value: Clone + Sized),+
        {
            type Value = ($($T::Value,)+);
            type ReadOutput<'a> = $guard<'a, $($T::ReadOutput<'a>, $T::Value),+> where Self: 'a;

            #[inline(always)]
            fn rx_track(&self) { $(self.$idx.rx_track();)+ }

            #[inline(always)]
            fn rx_read(&self) -> Option<Self::ReadOutput<'_>> {
                self.rx_track();
                self.rx_read_untracked()
            }

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                Some($guard(
                    $(self.$idx.rx_read_untracked()?,)+
                    OnceCell::new(),
                    PhantomData
                ))
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                let val = (
                    $(self.$idx.rx_try_with_untracked(|v| Clone::clone(v))?,)+
                );
                Some(fun(&val))
            }
            #[inline(always)] fn rx_defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> { None }
            #[inline(always)] fn rx_debug_name(&self) -> Option<String> { None }
            #[inline(always)] fn rx_is_disposed(&self) -> bool { $(self.$idx.rx_is_disposed() || )+ false }
            #[inline(always)] fn rx_is_constant(&self) -> bool { $(self.$idx.rx_is_constant() && )+ true }
        }
    };
}

impl_tuple_everything!(Tuple2ReadGuard, T0: 0, T1: 1; into);
impl_tuple_everything!(Tuple3ReadGuard, T0: 0, T1: 1, T2: 2; into);
impl_tuple_everything!(Tuple4ReadGuard, T0: 0, T1: 1, T2: 2, T3: 3; into);
impl_tuple_everything!(Tuple5ReadGuard, T0: 0, T1: 1, T2: 2, T3: 3, T4: 4);
impl_tuple_everything!(Tuple6ReadGuard, T0: 0, T1: 1, T2: 2, T3: 3, T4: 4, T5: 5);

impl<F, M> IntoRx for Rx<F, M>
where
    F: RxInternal + Clone + 'static,
    F::Value: Clone + 'static,
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

// --- ReactivityNode Helper Implementations ---

#[macro_export]
macro_rules! impl_rx_delegate {
    ($target:ident, $is_const:expr) => {
        impl<T: Clone + 'static> $crate::traits::IntoRx for $target<T> {
            type Value = T;
            type RxType = $crate::Rx<Self, $crate::RxValue>;
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
                $crate::reactivity::Signal::derive(move || $crate::traits::Read::get(&self))
            }
        }
    };
    ($target:ident, SignalID, $is_const:expr) => {
        impl<T: 'static> $crate::traits::ReactivityNode for $target<T> {
            type Value = T;
            #[inline(always)]
            fn node_id(&self) -> $crate::reactivity::NodeId {
                self.id
            }
        }

        impl<T: Clone + 'static> $crate::traits::IntoRx for $target<T> {
            type Value = T;
            type RxType = $crate::Rx<Self, $crate::RxValue>;
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
            type Value = T;
            type ReadOutput<'a>
                = $crate::traits::RxGuard<'a, T, T>
            where
                Self: 'a;

            #[inline(always)]
            fn rx_track(&self) {
                ::silex_reactivity::track_signal($crate::traits::ReactivityNode::node_id(self));
            }

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                let id = $crate::traits::ReactivityNode::node_id(self);
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
            fn rx_defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                ::silex_reactivity::get_node_defined_at($crate::traits::ReactivityNode::node_id(
                    self,
                ))
            }
            #[inline(always)]
            fn rx_debug_name(&self) -> Option<String> {
                ::silex_reactivity::get_debug_label($crate::traits::ReactivityNode::node_id(self))
            }
            #[inline(always)]
            fn rx_is_disposed(&self) -> bool {
                !::silex_reactivity::is_signal_valid($crate::traits::ReactivityNode::node_id(self))
            }
            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                $is_const
            }
        }
    };
    ($target:ident, $field:ident, $is_const:expr) => {
        impl<T: 'static> $crate::traits::ReactivityNode for $target<T> {
            type Value = T;
            #[inline(always)]
            fn node_id(&self) -> $crate::reactivity::NodeId {
                $crate::traits::ReactivityNode::node_id(&self.$field)
            }
        }

        impl<T: Clone + 'static> $crate::traits::IntoRx for $target<T> {
            type Value = T;
            type RxType = $crate::Rx<Self, $crate::RxValue>;
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
            type Value = T;
            type ReadOutput<'a>
                = <$crate::reactivity::ReadSignal<T> as $crate::traits::RxInternal>::ReadOutput<'a>
            where
                Self: 'a;

            #[inline(always)]
            fn rx_track(&self) {
                self.$field.rx_track();
            }

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                self.$field.rx_read_untracked()
            }

            #[inline(always)]
            fn rx_defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                self.$field.rx_defined_at()
            }
            #[inline(always)]
            fn rx_debug_name(&self) -> Option<String> {
                self.$field.rx_debug_name()
            }
            #[inline(always)]
            fn rx_is_disposed(&self) -> bool {
                self.$field.rx_is_disposed()
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
    ($target:ty, [$($gen:tt),*]) => {
        $crate::impl_reactive_op!($target, Add, add, [$($gen),*]);
        $crate::impl_reactive_op!($target, Sub, sub, [$($gen),*]);
        $crate::impl_reactive_op!($target, Mul, mul, [$($gen),*]);
        $crate::impl_reactive_op!($target, Div, div, [$($gen),*]);
        $crate::impl_reactive_op!($target, Rem, rem, [$($gen),*]);
        $crate::impl_reactive_op!($target, BitAnd, bitand, [$($gen),*]);
        $crate::impl_reactive_op!($target, BitOr, bitor, [$($gen),*]);
        $crate::impl_reactive_op!($target, BitXor, bitxor, [$($gen),*]);
        $crate::impl_reactive_op!($target, Shl, shl, [$($gen),*]);
        $crate::impl_reactive_op!($target, Shr, shr, [$($gen),*]);

        $crate::impl_reactive_unary_op!($target, Neg, neg, [$($gen),*]);
        $crate::impl_reactive_unary_op!($target, Not, not, [$($gen),*]);
    };
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
            for<'a> &'a T: std::ops::$trait<&'a T, Output = T>,
            T: Clone + 'static,
            R: $crate::traits::IntoRx<Value = T> + 'static,
            R::RxType: $crate::traits::RxInternal<Value = T> + Clone + 'static,
        {
            type Output = $crate::Rx<$crate::reactivity::OpPayload<T>, $crate::RxValue>;

            #[inline]
            fn $method(self, rhs: R) -> Self::Output {
                let lhs = self.into_signal();
                let rhs = rhs.into_signal();

                #[inline(always)]
                unsafe fn read_impl<InnerT>(inputs: &[$crate::reactivity::NodeId]) -> Option<InnerT>
                where
                    for<'a> &'a InnerT: std::ops::$trait<&'a InnerT, Output = InnerT>,
                    InnerT: 'static,
                {
                    unsafe {
                        let a = $crate::reactivity::rx_borrow_signal_unsafe::<InnerT>(inputs[0])?;
                        let b = $crate::reactivity::rx_borrow_signal_unsafe::<InnerT>(inputs[1])?;
                        Some($crate::traits::impls::ops_impl::$method(a, b))
                    }
                }

                let is_const = lhs.is_constant() && rhs.is_constant();

                $crate::Rx(
                    $crate::reactivity::OpPayload {
                        inputs: [lhs.ensure_node_id(), rhs.ensure_node_id()],
                        input_count: 2,
                        read: read_impl::<T>,
                        track: $crate::reactivity::op_trampolines::track_inputs,
                        is_constant: is_const,
                    },
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
            for<'a> &'a T: std::ops::$trait<Output = T>,
            T: Clone + 'static,
        {
            type Output = $crate::Rx<$crate::reactivity::OpPayload<T>, $crate::RxValue>;

            #[inline]
            fn $method(self) -> Self::Output {
                let val = self.into_signal();

                #[inline(always)]
                unsafe fn read_impl<InnerT>(inputs: &[$crate::reactivity::NodeId]) -> Option<InnerT>
                where
                    for<'a> &'a InnerT: std::ops::$trait<Output = InnerT>,
                    InnerT: 'static,
                {
                    unsafe {
                        let a = $crate::reactivity::rx_borrow_signal_unsafe::<InnerT>(inputs[0])?;
                        Some($crate::traits::impls::ops_impl::$method(a))
                    }
                }

                let is_const = val.is_constant();

                $crate::Rx(
                    $crate::reactivity::OpPayload {
                        inputs: [val.ensure_node_id(), val.ensure_node_id()],
                        input_count: 1,
                        read: read_impl::<T>,
                        track: $crate::reactivity::op_trampolines::track_inputs,
                        is_constant: is_const,
                    },
                    ::core::marker::PhantomData,
                )
            }
        }
    };
}

#[macro_export]
macro_rules! impl_reactive_op {
    ($target:ty, $trait:ident, $method:ident, [$($gen:tt),*]) => {
        impl<$($gen),*, R> std::ops::$trait<R> for $target
        where
            Self: $crate::traits::IntoRx,
            <Self as $crate::traits::IntoRx>::RxType: std::ops::$trait<R>,
        {
            type Output =
                <<Self as $crate::traits::IntoRx>::RxType as std::ops::$trait<R>>::Output;

            #[inline(always)]
            fn $method(self, rhs: R) -> Self::Output {
                $crate::traits::IntoRx::into_rx(self).$method(rhs)
            }
        }
    };
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
    ($target:ty, $trait:ident, $method:ident, [$($gen:tt),*]) => {
        impl<$($gen),*> std::ops::$trait for $target
        where
            Self: $crate::traits::IntoRx,
            <Self as $crate::traits::IntoRx>::RxType: std::ops::$trait,
        {
            type Output =
                <<Self as $crate::traits::IntoRx>::RxType as std::ops::$trait>::Output;

            #[inline(always)]
            fn $method(self) -> Self::Output {
                $crate::traits::IntoRx::into_rx(self).$method()
            }
        }
    };
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
    <S as With>::Value: PartialEq + Sized + 'static,
{
}

impl<S> ReactivePartialOrd for S
where
    S: With + Clone + 'static,
    <S as With>::Value: PartialOrd + Sized + 'static,
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
