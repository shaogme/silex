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

            $crate::Rx::new_pooled(::silex_reactivity::store_value(Box::new(move || {
                $crate::logic::arithmetic::ops_impl::$fn_impl(&lhs.get(), &rhs.get())
            })
                as Box<dyn Fn() -> bool>))
        }
    };
}

/// Provides a fluent API for checking equality on reactive values.
pub trait ReactivePartialEq: RxRead + Clone + 'static
where
    Self::Value: PartialEq + Sized + 'static,
{
    crate::reactive_compare_method!(equals, eq, ==, PartialEq);
    crate::reactive_compare_method!(not_equals, ne, !=, PartialEq);
}

/// Provides a fluent API for checking ordering on reactive values.
pub trait ReactivePartialOrd: RxRead + Clone + 'static
where
    Self::Value: PartialOrd + Sized + 'static,
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
{
}

impl<S> ReactivePartialOrd for S
where
    S: RxRead + Clone + 'static,
    S::Value: PartialOrd + Sized + 'static,
{
}
