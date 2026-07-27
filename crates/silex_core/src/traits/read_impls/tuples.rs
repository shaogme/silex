use crate::{
    Rx, RxValueKind,
    reactivity::{RawId, Signal, StaticMap2Payload, StaticMapPayload, op_trampolines},
    traits::{IntoRx, IntoSignal, RxBase, RxCloneData, RxData, RxGet, RxGuard, RxInternal},
};

pub fn create_tuple2_rx<I1: RxData, I2: RxData>(
    ids: [RawId; 2],
    mapper: fn(&I1, &I2) -> (I1, I2),
    is_constant: bool,
) -> Rx<(I1, I2)> {
    let op = StaticMap2Payload::new2(ids, mapper, is_constant);
    Rx::new_op(op)
}

pub fn create_tuple_n_rx<const N: usize, V: RxCloneData>(
    ids: [RawId; N],
    mapper: fn(&[RawId; N]) -> V,
    is_constant: bool,
) -> Rx<V> {
    let ids_vec = ids.to_vec();
    let meta_id =
        silex_reactivity::scope::untrack(|| silex_reactivity::store::create(ids_vec)).raw();
    // Important: for TupleN we need track_tuple_meta_slice as track trampoline
    let op = StaticMapPayload::<V>::new1_with_track_and_compute(
        meta_id,
        mapper as *const (),
        op_trampolines::compute_tuple_meta::<N, V>,
        op_trampolines::track_tuple_meta_slice,
        is_constant,
    );
    Rx::new_op(op)
}

macro_rules! impl_tuple_into_rx {
    // 专用 1 元元组分支
    (1, $T0:ident : $idx0:tt) => {
        impl<$T0> $crate::traits::RxValue for ($T0,)
        where
            $T0: $crate::traits::RxValue,
            $T0::Value: Sized,
        {
            type Value = ($T0::Value,);
        }

        impl<$T0> IntoRx for ($T0,)
        where
            $T0: IntoRx + IntoSignal + $crate::traits::RxCloneData,
            $T0::Value: $crate::traits::RxCloneData,
        {
            type RxType = Rx<Self::Value, RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                let sig = self.$idx0.into_signal();
                Rx::derive(Box::new(move || {
                    #[allow(unused_imports)]
                    use $crate::traits::RxRead;
                    (sig.get(),)
                }))
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                self.$idx0.is_constant()
            }
        }

        impl<$T0> IntoSignal for ($T0,)
        where
            $T0: IntoRx + IntoSignal + $crate::traits::RxCloneData,
            $T0::Value: $crate::traits::RxCloneData,
        {
            #[inline(always)]
            fn into_signal(self) -> Signal<Self::Value>
            where
                Self: 'static,
            {
                Signal::derive(Box::new(move || self.clone().into_rx().get()))
            }
        }

        impl<$T0> RxBase for ($T0,)
        where
            $T0: RxBase,
            $T0::Value: Sized,
        {
            fn raw_id(&self) -> Option<RawId> {
                self.$idx0.raw_id()
            }
            fn track(&self) {
                self.$idx0.track();
            }
            fn is_disposed(&self) -> bool {
                self.$idx0.is_disposed()
            }
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                self.$idx0.defined_at()
            }
            fn debug_name(&self) -> Option<String> {
                self.$idx0.debug_name()
            }
        }

        impl<$T0> RxInternal for ($T0,)
        where
            $T0: RxInternal + $crate::traits::RxData,
            $T0::Value: Sized + $crate::traits::RxCloneData,
        {
            type ReadOutput<'a> = RxGuard<'a, Self::Value, Self::Value>
            where
                Self: 'a;
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                Some(RxGuard::Owned(self.rx_get_adaptive()?))
            }
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                self.rx_get_adaptive().map(|v| fun(&v))
            }
            fn rx_get_adaptive(&self) -> Option<Self::Value>
            where
                Self::Value: Sized,
            {
                Some((self.$idx0.rx_get_adaptive()?,))
            }
            fn rx_is_constant(&self) -> bool {
                self.$idx0.rx_is_constant()
            }
        }
    };

    // 专用 2 元元组分支
    (2, $T0:ident : $idx0:tt, $T1:ident : $idx1:tt) => {
        impl<$T0, $T1> $crate::traits::RxValue for ($T0, $T1)
        where $T0: $crate::traits::RxValue, $T1: $crate::traits::RxValue,
              $T0::Value: Sized, $T1::Value: Sized
        {
            type Value = ($T0::Value, $T1::Value);
        }

        impl<$T0, $T1> IntoRx for ($T0, $T1)
        where
            $T0: IntoRx + IntoSignal + $crate::traits::RxCloneData,
            $T1: IntoRx + IntoSignal + $crate::traits::RxCloneData,
            $T0::Value: $crate::traits::RxCloneData,
            $T1::Value: $crate::traits::RxCloneData,
        {
            type RxType = Rx<Self::Value, RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                let ids = [
                    self.$idx0.clone().into_signal().ensure_raw_id(),
                    self.$idx1.clone().into_signal().ensure_raw_id(),
                ];
                $crate::traits::read_impls::create_tuple2_rx::<
                    $T0::Value,
                    $T1::Value,
                >(
                    ids,
                    $crate::reactivity::op_trampolines::tuple_2_mapper::<$T0::Value, $T1::Value>,
                    self.is_constant(),
                )
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                self.$idx0.is_constant() && self.$idx1.is_constant()
            }
        }

        impl<$T0, $T1> IntoSignal for ($T0, $T1)
        where
            $T0: IntoRx + IntoSignal + $crate::traits::RxCloneData,
            $T0::Value: $crate::traits::RxCloneData,
            $T1: IntoRx + IntoSignal + $crate::traits::RxCloneData,
            $T1::Value: $crate::traits::RxCloneData,
        {
            #[inline(always)]
            fn into_signal(self) -> Signal<Self::Value> where Self: 'static {
                Signal::derive(Box::new(move || self.clone().into_rx().get()))
            }
        }

        impl<$T0, $T1> RxBase for ($T0, $T1)
        where $T0: RxBase, $T1: RxBase, $T0::Value: Sized, $T1::Value: Sized
        {
            fn raw_id(&self) -> Option<RawId> { None }
            fn track(&self) { self.$idx0.track(); self.$idx1.track(); }
            fn is_disposed(&self) -> bool { self.$idx0.is_disposed() || self.$idx1.is_disposed() }
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> { None }
            fn debug_name(&self) -> Option<String> { None }
        }

        impl<$T0, $T1> RxInternal for ($T0, $T1)
        where
            $T0: RxInternal + $crate::traits::RxData + IntoRx,
            $T1: RxInternal + $crate::traits::RxData + IntoRx,
            $T0::Value: $crate::traits::RxCloneData,
            $T1::Value: $crate::traits::RxCloneData,
        {
            type ReadOutput<'a> = RxGuard<'a, Self::Value, Self::Value> where Self: 'a;
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> { Some(RxGuard::Owned(self.rx_get_adaptive()?)) }
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> { self.rx_get_adaptive().map(|v| fun(&v)) }
            fn rx_get_adaptive(&self) -> Option<Self::Value> where Self::Value: Sized { Some((self.$idx0.rx_get_adaptive()?, self.$idx1.rx_get_adaptive()?)) }
            fn rx_is_constant(&self) -> bool { self.$idx0.rx_is_constant() && self.$idx1.rx_is_constant() }
        }
    };

    // 多元元组分支 (N > 2)
    ($len:expr, $trap:ident, $($T:ident : $idx:tt),+) => {
        impl<$($T),+> $crate::traits::RxValue for ($($T,)+)
        where $($T: $crate::traits::RxValue),+, $($T::Value: Sized),+
        {
            type Value = ($($T::Value,)+);
        }

        impl<$($T),+> IntoRx for ($($T,)+)
        where
            $($T: IntoRx + IntoSignal + $crate::traits::RxCloneData),+,
            $($T::Value: $crate::traits::RxCloneData),+
        {
            type RxType = Rx<Self::Value, RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                let ids = [$(self.$idx.clone().into_signal().ensure_raw_id()),+];
                $crate::traits::read_impls::create_tuple_n_rx::<$len, Self::Value>(
                    ids,
                    $crate::reactivity::op_trampolines::$trap::<$($T::Value),+>,
                    self.is_constant(),
                )
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                $(self.$idx.is_constant() && )+ true
            }
        }

        impl<$($T),+> IntoSignal for ($($T,)+)
        where
            $($T: IntoRx + IntoSignal + $crate::traits::RxCloneData),+,
            $($T::Value: $crate::traits::RxCloneData),+
        {
            #[inline(always)]
            fn into_signal(self) -> Signal<Self::Value> where Self: 'static {
                Signal::derive(Box::new(move || self.clone().into_rx().get()))
            }
        }

        impl<$($T),+> RxBase for ($($T,)+)
        where $($T: RxBase),+, $($T::Value: Sized),+
        {
            fn raw_id(&self) -> Option<RawId> { None }
            fn track(&self) { $(self.$idx.track();)+ }
            fn is_disposed(&self) -> bool { $(self.$idx.is_disposed() || )+ false }
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> { None }
            fn debug_name(&self) -> Option<String> { None }
        }

        impl<$($T),+> RxInternal for ($($T,)+)
        where
            $($T: RxInternal + $crate::traits::RxData),+, $($T: IntoRx),+,
            $($T::Value: $crate::traits::RxCloneData),+
        {
            type ReadOutput<'a> = RxGuard<'a, Self::Value, Self::Value> where Self: 'a;
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> { Some(RxGuard::Owned(self.rx_get_adaptive()?)) }
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> { self.rx_get_adaptive().map(|v| fun(&v)) }
            fn rx_get_adaptive(&self) -> Option<Self::Value> where Self::Value: Sized { Some(($(self.$idx.rx_get_adaptive()?,)+)) }
            fn rx_is_constant(&self) -> bool { $(self.$idx.rx_is_constant() && )+ true }
        }
    };
}

impl_tuple_into_rx!(1, T0: 0);
impl_tuple_into_rx!(2, T0: 0, T1: 1);
impl_tuple_into_rx!(3, tuple_3_mapper, T0: 0, T1: 1, T2: 2);
impl_tuple_into_rx!(4, tuple_4_mapper, T0: 0, T1: 1, T2: 2, T3: 3);
impl_tuple_into_rx!(5, tuple_5_mapper, T0: 0, T1: 1, T2: 2, T3: 3, T4: 4);
impl_tuple_into_rx!(6, tuple_6_mapper, T0: 0, T1: 1, T2: 2, T3: 3, T4: 4, T5: 5);
