//! Trait implementations for reactive tuples.

use crate::{
    SilexResult,
    reactivity::{
        TupleReadGuard1, TupleReadGuard2, TupleReadGuard3, TupleReadGuard4, TupleReadGuard5,
        TupleReadGuard6,
    },
    traits::{
        RxBase, RxData, RxGet, RxRead, RxReadRefSource, RxReadTuple1, RxReadTuple2, RxReadTuple3,
        RxReadTuple4, RxReadTuple5, RxReadTuple6, RxReadTupleSource1, RxReadTupleSource2,
        RxReadTupleSource3, RxReadTupleSource4, RxReadTupleSource5, RxReadTupleSource6, RxValue,
    },
};

impl<T> RxValue for (T,)
where
    T: RxValue,
    T::Owned: Sized + RxData,
{
    type Owned = (T::Owned,);
}

macro_rules! impl_tuple_rx_value {
    ($($name:ident),+ $(,)?) => {
        impl<$($name),+> RxValue for ($($name,)+)
        where
            $($name: RxValue, $name::Owned: Sized + RxData),+
        {
            type Owned = ($($name::Owned,)+);
        }
    };
}

impl_tuple_rx_value!(A, B);
impl_tuple_rx_value!(A, B, C);
impl_tuple_rx_value!(A, B, C, D);
impl_tuple_rx_value!(A, B, C, D, E);
impl_tuple_rx_value!(A, B, C, D, E, F);

macro_rules! impl_tuple_rx_traits {
    (
        $source_trait:ident,
        $read_trait:ident,
        $guard:ident,
        $($name:ident : $var:ident : $index:tt),+ $(,)?
    ) => {
        impl<$($name),+> RxBase for ($($name,)+)
        where
            $($name: RxBase, $name: RxValue, $name::Owned: Sized + RxData,)+
        {
            fn track(&self) -> SilexResult<()> {
                $(self.$index.track()?;)+
                Ok(())
            }
        }

        impl<$($name),+> RxRead for ($($name,)+)
        where
            $($name: RxValue + RxReadRefSource,)+
            $($name::Owned: Sized + RxData,)+
        {
            type ReadGuard<'a>
                = $guard<$($name::ViewGuard<'a>),+>
            where
                Self: 'a;

            fn read<'a>(&'a self) -> SilexResult<Self::ReadGuard<'a>> {
                Ok($guard::new($(self.$index.read_ref()?),+))
            }

            fn read_untracked<'a>(&'a self) -> SilexResult<Self::ReadGuard<'a>> {
                Ok($guard::new($(self.$index.read_ref_untracked()?),+))
            }
        }

        impl<$($name),+> $source_trait<$($name::Owned),+> for ($($name,)+)
        where
            $($name: RxValue + RxReadRefSource,)+
            $($name::Owned: Sized + RxData,)+
        {
            type ViewGuard<'a>
                = $guard<$($name::ViewGuard<'a>),+>
            where
                Self: 'a;

            fn read_tuple<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
                self.read()
            }

            fn read_tuple_untracked<'a>(&'a self) -> SilexResult<Self::ViewGuard<'a>> {
                self.read_untracked()
            }
        }

        impl<S, $($name),+> $read_trait<$($name),+> for S
        where
            S: RxRead<Owned = ($($name,)+)> + $source_trait<$($name),+>,
        {}

        impl<$($name),+> RxGet for ($($name,)+)
        where
            $($name: RxValue,)+
            ($($name,)+): $read_trait<$($name::Owned),+>,
            $($name::Owned: Clone,)+
        {
            fn get_untracked(&self) -> SilexResult<Self::Owned> {
                self.with_untracked(|($($var,)+)| ($($var.clone(),)+))
            }

            fn get(&self) -> SilexResult<Self::Owned> {
                self.with(|($($var,)+)| ($($var.clone(),)+))
            }
        }
    };
}

impl_tuple_rx_traits!(RxReadTupleSource1, RxReadTuple1, TupleReadGuard1, A: a: 0);
impl_tuple_rx_traits!(RxReadTupleSource2, RxReadTuple2, TupleReadGuard2, A: a: 0, B: b: 1);
impl_tuple_rx_traits!(
    RxReadTupleSource3,
    RxReadTuple3,
    TupleReadGuard3,
    A: a: 0,
    B: b: 1,
    C: c: 2
);
impl_tuple_rx_traits!(
    RxReadTupleSource4,
    RxReadTuple4,
    TupleReadGuard4,
    A: a: 0,
    B: b: 1,
    C: c: 2,
    D: d: 3
);
impl_tuple_rx_traits!(
    RxReadTupleSource5,
    RxReadTuple5,
    TupleReadGuard5,
    A: a: 0,
    B: b: 1,
    C: c: 2,
    D: d: 3,
    E: e: 4
);
impl_tuple_rx_traits!(
    RxReadTupleSource6,
    RxReadTuple6,
    TupleReadGuard6,
    A: a: 0,
    B: b: 1,
    C: c: 2,
    D: d: 3,
    E: e: 4,
    F: f: 5
);
