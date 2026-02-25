use crate::traits::RxRead;
use std::ops::Deref;

pub type CompareFn<T> = fn(&T, &T) -> bool;

#[doc(hidden)]
#[macro_export]
macro_rules! reactive_compare_method {
    ($name:ident, $fn_impl:ident, $op:tt, $bound:ident) => {
        fn $name<O>(
            &self,
            other: O,
        ) -> $crate::Rx<$crate::reactivity::OpPayload<bool, 2>, $crate::RxValue>
        where
            Self: $crate::traits::IntoRx,
            Self::RxType: $crate::traits::RxInternal<Value = <Self as $crate::traits::IntoRx>::Value> + Clone + 'static,
            O: $crate::traits::IntoRx<Value = <Self as $crate::traits::IntoRx>::Value> + 'static,
            O::RxType: $crate::traits::RxInternal<Value = <Self as $crate::traits::IntoRx>::Value> + Clone + 'static,
            <Self as $crate::traits::IntoRx>::Value: $bound + Sized + Clone + 'static,
        {
            let lhs = self.clone().into_signal();
            let rhs = other.into_signal();

            #[inline(always)]
            unsafe fn read_impl<InnerT: $bound + 'static>(inputs: &[$crate::reactivity::NodeId]) -> Option<bool> {
                unsafe {
                    let a = $crate::reactivity::rx_borrow_signal_unsafe::<InnerT>(inputs[0])?;
                    let b = $crate::reactivity::rx_borrow_signal_unsafe::<InnerT>(inputs[1])?;
                    Some($crate::logic::arithmetic::ops_impl::$fn_impl(a, b))
                }
            }

            let is_const = lhs.is_constant() && rhs.is_constant();

            $crate::Rx(
                $crate::reactivity::OpPayload {
                    inputs: [lhs.ensure_node_id(), rhs.ensure_node_id()],
                    read: read_impl::<<Self as $crate::traits::IntoRx>::Value>,
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
    crate::reactive_compare_method!(equals, eq, ==, PartialEq);
    crate::reactive_compare_method!(not_equals, ne, !=, PartialEq);
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: RxRead + Clone + 'static
where
    Self::Value: PartialOrd + Sized + 'static,
    for<'a> Self::ReadOutput<'a>: Deref<Target = Self::Value>,
{
    crate::reactive_compare_method!(greater_than, gt, >, PartialOrd);
    crate::reactive_compare_method!(less_than, lt, <, PartialOrd);
    crate::reactive_compare_method!(greater_than_or_equals, ge, >=, PartialOrd);
    crate::reactive_compare_method!(less_than_or_equals, le, <=, PartialOrd);
}

impl<S> ReactivePartialEq for S
where
    S: RxRead + Clone + 'static,
    S::Value: PartialEq + Sized + 'static,
    for<'a> S::ReadOutput<'a>: Deref<Target = S::Value>,
{
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone + 'static,
    S::Value: PartialOrd + Sized + 'static,
    for<'a> S::ReadOutput<'a>: Deref<Target = S::Value>,
{
}
