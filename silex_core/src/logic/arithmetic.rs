/// A module containing static helper functions for reactive operations.
/// usage of these avoids generating unique closures for every operator implementation.
#[doc(hidden)]
pub mod ops_impl {
    use crate::reactivity::NodeId;
    use crate::reactivity::rx_borrow_signal_unsafe;
    use std::ops::*;

    macro_rules! gen_ops {
        (bin: $($fn:ident:$trait:ident:$trap:ident),*; un: $($ufn:ident:$utrait:ident:$utrap:ident),*) => {
            $(
                #[inline]
                pub fn $fn<T>(lhs: &T, rhs: &T) -> T
                where
                    for<'a> &'a T: $trait<&'a T, Output = T>,
                {
                    lhs.$fn(rhs)
                }

                pub unsafe fn $trap<T>(inputs: &[NodeId]) -> Option<T>
                where
                    for<'a> &'a T: $trait<&'a T, Output = T>,
                    T: $crate::traits::RxData,
                {
                    unsafe {
                        let lhs = rx_borrow_signal_unsafe::<T>(inputs[0])?;
                        let rhs = rx_borrow_signal_unsafe::<T>(inputs[1])?;
                        Some(lhs.$fn(rhs))
                    }
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

                pub unsafe fn $utrap<T>(inputs: &[NodeId]) -> Option<T>
                where
                    for<'a> &'a T: $utrait<Output = T>,
                    T: $crate::traits::RxData,
                {
                    unsafe {
                        let val = rx_borrow_signal_unsafe::<T>(inputs[0])?;
                        Some(val.$ufn())
                    }
                }
            )*
        };
    }

    gen_ops!(
        bin: add:Add:add_t, sub:Sub:sub_t, mul:Mul:mul_t, div:Div:div_t, rem:Rem:rem_t,
             bitand:BitAnd:bitand_t, bitor:BitOr:bitor_t, bitxor:BitXor:bitxor_t,
             shl:Shl:shl_t, shr:Shr:shr_t;
        un: neg:Neg:neg_t, not:Not:not_t
    );

    macro_rules! gen_cmp_ops {
        ($($fn:ident:$op:tt:$bound:ident:$trap:ident),*) => {
            $(
                #[inline]
                pub fn $fn<T>(lhs: &T, rhs: &T) -> bool
                where
                    T: $bound,
                {
                    lhs $op rhs
                }

                pub unsafe fn $trap<T>(inputs: &[NodeId]) -> Option<bool>
                where
                    T: $bound + $crate::traits::RxData,
                {
                    unsafe {
                        let lhs = rx_borrow_signal_unsafe::<T>(inputs[0])?;
                        let rhs = rx_borrow_signal_unsafe::<T>(inputs[1])?;
                        Some(lhs $op rhs)
                    }
                }
            )*
        };
    }
    gen_cmp_ops!(
        eq:==:PartialEq:eq_t, ne:!=:PartialEq:ne_t,
        gt:>:PartialOrd:gt_t, lt:<:PartialOrd:lt_t, ge:>=:PartialOrd:ge_t, le:<=:PartialOrd:le_t
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
        $crate::impl_rx_op!(Add, add, add_t);
        $crate::impl_rx_op!(Sub, sub, sub_t);
        $crate::impl_rx_op!(Mul, mul, mul_t);
        $crate::impl_rx_op!(Div, div, div_t);
        $crate::impl_rx_op!(Rem, rem, rem_t);
        $crate::impl_rx_op!(BitAnd, bitand, bitand_t);
        $crate::impl_rx_op!(BitOr, bitor, bitor_t);
        $crate::impl_rx_op!(BitXor, bitxor, bitxor_t);
        $crate::impl_rx_op!(Shl, shl, shl_t);
        $crate::impl_rx_op!(Shr, shr, shr_t);

        $crate::impl_rx_unary_op!(Neg, neg, neg_t);
        $crate::impl_rx_unary_op!(Not, not, not_t);
    };
}

#[macro_export]
macro_rules! impl_rx_op {
    ($trait:ident, $method:ident, $trap:ident) => {
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

                let op = $crate::reactivity::OpPayload::new(
                    [lhs.ensure_node_id(), rhs.ensure_node_id()],
                    $crate::logic::arithmetic::ops_impl::$trap::<T>,
                    $crate::reactivity::op_trampolines::track_inputs,
                    false,
                );
                $crate::Rx::new_op(op)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_rx_unary_op {
    ($trait:ident, $method:ident, $trap:ident) => {
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

                let op = $crate::reactivity::OpPayload::new(
                    [val.ensure_node_id()],
                    $crate::logic::arithmetic::ops_impl::$trap::<T>,
                    $crate::reactivity::op_trampolines::track_inputs,
                    false,
                );
                $crate::Rx::new_op(op)
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
