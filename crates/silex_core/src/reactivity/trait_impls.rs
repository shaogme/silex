use crate::{
    OwnedReadGuard, SilexResult,
    traits::{RxBase, RxData, RxRead, RxValue},
};

macro_rules! impl_primitive_rx_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RxValue for $ty {
                type Value = Self;
            }
        )*
    };
}

impl_primitive_rx_value!(
    (),
    bool,
    char,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    String,
);

impl RxValue for &str {
    type Value = String;
}

impl<T> RxValue for (T,)
where
    T: RxValue,
    T::Value: Sized + RxData,
{
    type Value = (T::Value,);
}

macro_rules! impl_tuple_rx_value {
    ($($name:ident),+ $(,)?) => {
        impl<$($name),+> RxValue for ($($name,)+)
        where
            $($name: RxValue, $name::Value: Sized + RxData),+
        {
            type Value = ($($name::Value,)+);
        }
    };
}

impl_tuple_rx_value!(A, B);
impl_tuple_rx_value!(A, B, C);
impl_tuple_rx_value!(A, B, C, D);
impl_tuple_rx_value!(A, B, C, D, E);
impl_tuple_rx_value!(A, B, C, D, E, F);

/// Aggregate dependency tracking and clone-backed reads for reactive tuples.
macro_rules! impl_tuple_rx_traits {
    ($($name:ident : $index:tt),+ $(,)?) => {
        impl<$($name),+> RxBase for ($($name,)+)
        where
            $($name: RxBase, $name::Value: Sized + RxData,)+
        {
            fn track(&self) -> SilexResult<()> {
                $(self.$index.track()?;)+
                Ok(())
            }
        }

        impl<$($name),+> RxRead for ($($name,)+)
        where
            $($name: RxRead, $name::Value: Sized + Clone + RxData),+
        {
            type ReadGuard<'a> = OwnedReadGuard<Self::Value> where Self: 'a;

            fn read(&self) -> SilexResult<Self::ReadGuard<'_>> {
                let value = ($(self.$index.with(|value| (*value).clone())?,)+);
                Ok(OwnedReadGuard::new(value))
            }

            fn read_untracked(&self) -> SilexResult<Self::ReadGuard<'_>> {
                let value = ($(self.$index.with_untracked(|value| (*value).clone())?,)+);
                Ok(OwnedReadGuard::new(value))
            }

            fn with<U>(&self, f: impl FnOnce(&Self::Value) -> U) -> SilexResult<U> {
                let value = ($(self.$index.with(|value| (*value).clone())?,)+);
                Ok(f(&value))
            }

            fn with_untracked<U>(
                &self,
                f: impl FnOnce(&Self::Value) -> U,
            ) -> SilexResult<U> {
                let value = ($(self.$index.with_untracked(|value| (*value).clone())?,)+);
                Ok(f(&value))
            }
        }
    };
}

impl_tuple_rx_traits!(A: 0);
impl_tuple_rx_traits!(A: 0, B: 1);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2, D: 3);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2, D: 3, E: 4);
impl_tuple_rx_traits!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
