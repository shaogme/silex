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
                use $crate::traits::{IntoSignal, RxGet};

                let lhs = self.into_signal();
                let rhs = rhs.into_signal();

                if lhs.is_constant() && rhs.is_constant() {
                    return $crate::Rx::new_constant($crate::logic::arithmetic::ops_impl::$method(
                        &lhs.get(),
                        &rhs.get(),
                    ));
                }

                $crate::Rx::new_pooled(::silex_reactivity::store_value(Box::new(move || {
                    $crate::logic::arithmetic::ops_impl::$method(&lhs.get(), &rhs.get())
                })
                    as Box<dyn Fn() -> T>))
            }
        }
    };
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
                use $crate::traits::{IntoSignal, RxGet};

                let val = self.into_signal();

                if val.is_constant() {
                    return $crate::Rx::new_constant($crate::logic::arithmetic::ops_impl::$method(
                        &val.get(),
                    ));
                }

                $crate::Rx::new_pooled(::silex_reactivity::store_value(Box::new(move || {
                    $crate::logic::arithmetic::ops_impl::$method(&val.get())
                })
                    as Box<dyn Fn() -> T>))
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
