use crate::traits::RxRead;

pub type CompareFn<T> = fn(&T, &T) -> bool;

#[doc(hidden)]
#[macro_export]
macro_rules! reactive_compare_method {
    ($name:ident, $fn_impl:ident, $op:tt, $bound:ident) => {
        fn $name<O>(&self, other: O) -> $crate::Rx<bool, $crate::RxValueKind>
        where
            Self: $crate::traits::IntoRx + $crate::traits::IntoSignal + $crate::traits::RxValue,
            O: $crate::traits::IntoRx
                + $crate::traits::IntoSignal
                + $crate::traits::RxValue<Value = Self::Value>
                + 'static,
            Self::Value: $bound + Sized + $crate::traits::RxCloneData + 'static,
        {
            use $crate::traits::RxGet;
            let lhs = self.clone().into_signal();
            let rhs = other.into_signal();

            if lhs.is_constant() && rhs.is_constant() {
                return $crate::Rx::new_constant($crate::logic::arithmetic::ops_impl::$fn_impl(
                    &lhs.get(),
                    &rhs.get(),
                ));
            }

            let op = $crate::reactivity::StaticMap2Payload::new(
                [lhs.ensure_node_id(), rhs.ensure_node_id()],
                $crate::logic::arithmetic::ops_impl::$fn_impl::<Self::Value>,
                false,
            );
            $crate::Rx::new_op_raw(op)
        }
    };
}

/// Provides a fluent API for checking equality on reactive values.
pub trait ReactivePartialEq: RxRead + Clone + 'static
where
    Self::Value: PartialEq + Sized + 'static,
{
    reactive_compare_method!(equals, eq, ==, PartialEq);
    reactive_compare_method!(not_equals, ne, !=, PartialEq);
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: RxRead + Clone + 'static
where
    Self::Value: PartialOrd + Sized + 'static,
{
    reactive_compare_method!(greater_than, gt, >, PartialOrd);
    reactive_compare_method!(less_than, lt, <, PartialOrd);
    reactive_compare_method!(greater_than_or_equals, ge, >=, PartialOrd);
    reactive_compare_method!(less_than_or_equals, le, <=, PartialOrd);
}

impl<S> ReactivePartialEq for S
where
    S: RxRead + Clone + 'static,
    S::Value: PartialEq + Sized + 'static,
{
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone + 'static,
    S::Value: PartialOrd + Sized + 'static,
{
}
