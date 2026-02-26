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
        bin: add:Add, sub:Sub, mul:Mul, div:Div, rem:Rem,
             bitand:BitAnd, bitor:BitOr, bitxor:BitXor,
             shl:Shl, shr:Shr;
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
        impl<R, T> std::ops::$trait<R> for $crate::Rx<T, $crate::RxValueKind>
        where
            for<'a> &'a T: std::ops::$trait<&'a T, Output = T>,
            T: $crate::traits::RxCloneData + 'static,
            R: $crate::traits::IntoRx<Value = T> + $crate::traits::IntoSignal + 'static,
        {
            type Output = $crate::Rx<T, $crate::RxValueKind>;

            #[inline]
            fn $method(self, rhs: R) -> Self::Output {
                $crate::logic::arithmetic::apply_binary_op::<T, R>(
                    self,
                    rhs,
                    $crate::logic::arithmetic::ops_impl::$method::<T>,
                )
            }
        }
    };
}

pub fn apply_binary_op<T, R>(lhs: crate::Rx<T>, rhs: R, f: fn(&T, &T) -> T) -> crate::Rx<T>
where
    T: crate::traits::RxCloneData + 'static,
    R: crate::traits::IntoSignal<Value = T> + 'static,
{
    use crate::traits::{IntoSignal, RxGet};

    let lhs_s = lhs.into_signal();
    let rhs_s = rhs.into_signal();

    if lhs_s.is_constant() && rhs_s.is_constant() {
        return crate::Rx::new_constant(f(&lhs_s.get(), &rhs_s.get()));
    }

    let op = crate::reactivity::StaticMap2Payload::new(
        [lhs_s.ensure_node_id(), rhs_s.ensure_node_id()],
        f,
        false,
    );
    crate::Rx::new_op_raw(op)
}

#[macro_export]
macro_rules! impl_rx_unary_op {
    ($trait:ident, $method:ident) => {
        impl<T> std::ops::$trait for $crate::Rx<T, $crate::RxValueKind>
        where
            for<'a> &'a T: std::ops::$trait<Output = T>,
            T: $crate::traits::RxCloneData + 'static,
        {
            type Output = $crate::Rx<T, $crate::RxValueKind>;

            #[inline]
            fn $method(self) -> Self::Output {
                $crate::logic::arithmetic::apply_unary_op::<T>(
                    self,
                    $crate::logic::arithmetic::ops_impl::$method::<T>,
                )
            }
        }
    };
}

pub fn apply_unary_op<T>(val: crate::Rx<T>, f: fn(&T) -> T) -> crate::Rx<T>
where
    T: crate::traits::RxCloneData + 'static,
{
    use crate::traits::{IntoSignal, RxGet};

    let val_s = val.into_signal();

    if val_s.is_constant() {
        return crate::Rx::new_constant(f(&val_s.get()));
    }

    let op = crate::reactivity::StaticMapPayload::new_unary(val_s.ensure_node_id(), f, false);
    crate::Rx::new_op_raw(op)
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
