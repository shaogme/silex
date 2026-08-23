//! Trait implementations for primitive values and generic adapters.

use crate::{
    Callback, NodeRef, OwnerAccess, Rx, SilexError, SilexResult,
    reactivity::{Computed, MappedOptionReadGuard, ReadSignal, Signal, StoredValue},
    traits::{
        ForLoopSource, ReactiveInput, RxCloneData, RxData, RxDefault, RxError, RxFrom, RxOptionExt,
        RxRead, RxReadOption, RxReadOptionSource, RxReadRef, RxReadRefSource, RxValue,
    },
};
use std::fmt::Debug;

impl<T: ?Sized> RxData for T {}

impl<T: Clone> RxCloneData for T {}

impl<T: Clone + Debug> RxError for T {}

impl<'owner, T> RxDefault<'owner> for T where T: RxFrom<'owner> {}

macro_rules! impl_primitive_rx_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl RxValue for $ty {
                type Owned = Self;
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
    type Owned = String;
}

impl<S, T: ?Sized> RxReadRef<T> for S where S: RxRead<Owned = T> + RxReadRefSource {}

fn option_view<T>(value: &Option<T>) -> Option<&T> {
    value.as_ref()
}

impl<S, T> RxReadOptionSource<T> for S
where
    S: RxRead<Owned = Option<T>> + RxReadRefSource,
{
    type ViewGuard<'a>
        = MappedOptionReadGuard<S::ViewGuard<'a>, fn(&Option<T>) -> Option<&T>, Option<T>, T>
    where
        Self: 'a;

    fn read_option<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        Ok(MappedOptionReadGuard::new(
            self.read_ref()?,
            option_view::<T>,
        ))
    }

    fn read_option_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
        Ok(MappedOptionReadGuard::new(
            self.read_ref_untracked()?,
            option_view::<T>,
        ))
    }
}

impl<S, T> RxReadOption<T> for S where S: RxRead<Owned = Option<T>> + RxReadOptionSource<T> {}

impl<'owner, T: 'owner> RxFrom<'owner> for Callback<'owner, T> {
    type Owned = ();

    fn rx_from<V>(owner: OwnerAccess<'owner>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.callback(|_: T| Ok::<(), SilexError>(()))
    }
}

impl<'owner, T: 'owner> RxFrom<'owner> for NodeRef<'owner, T> {
    type Owned = ();

    fn rx_from<V>(owner: OwnerAccess<'owner>, _value: V) -> SilexResult<Self>
    where
        V: Into<Self::Owned>,
    {
        owner.node_ref()
    }
}

macro_rules! impl_reactive_input_values {
    ($($value:ty),* $(,)?) => {
        $(
            impl<'owner> ReactiveInput<'owner, ReadSignal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<ReadSignal<'owner, $value>> {
                    <ReadSignal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Signal<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Signal<'owner, $value>> {
                    <Signal<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Computed<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Computed<'owner, $value>> {
                    <Computed<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, StoredValue<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<StoredValue<'owner, $value>> {
                    <StoredValue<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner> ReactiveInput<'owner, Rx<'owner, $value>> for $value {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Rx<'owner, $value>> {
                    <Rx<'owner, $value> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }
        )*
    };
}

impl_reactive_input_values!(
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

macro_rules! impl_reactive_input_str_values {
    ($($target:ty),* $(,)?) => {
        $(
            impl<'owner, 'value> ReactiveInput<'owner, ReadSignal<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<ReadSignal<'owner, $target>> {
                    <ReadSignal<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Signal<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Signal<'owner, $target>> {
                    <Signal<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Computed<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Computed<'owner, $target>> {
                    <Computed<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, StoredValue<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<StoredValue<'owner, $target>> {
                    <StoredValue<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }

            impl<'owner, 'value> ReactiveInput<'owner, Rx<'owner, $target>>
                for &'value str
            {
                fn into_reactive_input(
                    self,
                    owner: OwnerAccess<'owner>,
                ) -> SilexResult<Rx<'owner, $target>> {
                    <Rx<'owner, $target> as RxFrom<'owner>>::rx_from(owner, self)
                }
            }
        )*
    };
}

impl_reactive_input_str_values!(String);

impl<S, T> RxOptionExt<T> for S where S: RxReadOption<T> + Clone {}

impl<T: Clone> ForLoopSource for Vec<T> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self)
    }
}

impl<T: Clone> ForLoopSource for Option<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        Ok(self.as_deref().unwrap_or_default())
    }
}

impl<T: Clone> ForLoopSource for SilexResult<Vec<T>> {
    type Item = T;

    fn as_slice(&self) -> SilexResult<&[T]> {
        match self {
            Ok(items) => Ok(items.as_slice()),
            Err(error) => Err(error.clone()),
        }
    }
}
