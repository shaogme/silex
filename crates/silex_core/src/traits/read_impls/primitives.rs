use crate::{
    Rx, RxValueKind,
    reactivity::Signal,
    traits::{IntoRx, IntoSignal, RxGuard, RxInternal},
};

macro_rules! impl_into_rx_primitive {
    ($($t:ty $(: $val:ty => $conv:expr)?),*) => {
        $(
            impl $crate::traits::RxValue for $t {
                type Value = impl_into_rx_primitive!(@type $t $(, $val)?);
            }

            impl IntoRx for $t {
                type RxType = Rx<Self::Value, RxValueKind>;

                #[inline(always)]
                fn into_rx(self) -> Self::RxType {
                    let val = impl_into_rx_primitive!(@val self $(, $conv)?);
                    Rx::new_constant(val)
                }

                #[inline(always)]
                fn is_constant(&self) -> bool {
                    true
                }
            }

            impl IntoSignal for $t {
                #[inline(always)]
                fn into_signal(self) -> Signal<Self::Value> {
                    Signal::from(impl_into_rx_primitive!(@val self $(, $conv)?))
                }
            }
        )*
    };
    (@type $t:ty) => { $t };
    (@type $t:ty, $val:ty) => { $val };
    (@val $self:ident) => { $self };
    (@val $self:ident, $conv:expr) => { ($conv)($self) };
}

impl_into_rx_primitive!(
    bool, char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64,
    String,
    &str : String => |s: &str| s.to_string()
);

impl IntoRx for () {
    type RxType = Rx<(), RxValueKind>;

    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx::new_constant(())
    }

    #[inline(always)]
    fn is_constant(&self) -> bool {
        true
    }
}

impl IntoSignal for () {
    #[inline(always)]
    fn into_signal(self) -> Signal<()> {
        Signal::from(())
    }
}

impl RxInternal for () {
    type ReadOutput<'a> = RxGuard<'a, (), ()>;

    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        Some(RxGuard::Owned(()))
    }

    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        Some(fun(&()))
    }

    fn rx_is_constant(&self) -> bool {
        true
    }
}
