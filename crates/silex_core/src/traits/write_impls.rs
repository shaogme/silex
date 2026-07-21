#[macro_export]
macro_rules! impl_rx_write_delegate {
    ($target:ident, SignalID) => {
        impl<T: $crate::traits::RxData> $crate::traits::RxWrite for $target<T> {
            #[inline(always)]
            fn rx_try_update_untracked<URet>(
                &self,
                fun: impl FnOnce(&mut Self::Value) -> URet,
            ) -> Option<URet> {
                ::silex_reactivity::try_update_signal_untracked(self.id, fun)
            }

            #[inline(always)]
            fn rx_notify(&self) {
                ::silex_reactivity::notify_signal(self.id);
            }
        }
    };
    ($target:ident, $field:ident) => {
        impl<T: $crate::traits::RxData> $crate::traits::RxWrite for $target<T> {
            #[inline(always)]
            fn rx_try_update_untracked<URet>(
                &self,
                fun: impl FnOnce(&mut Self::Value) -> URet,
            ) -> Option<URet> {
                $crate::traits::RxWrite::rx_try_update_untracked(&self.$field, fun)
            }

            #[inline(always)]
            fn rx_notify(&self) {
                $crate::traits::RxWrite::rx_notify(&self.$field);
            }
        }
    };
}
