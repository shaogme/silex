#[macro_export]
macro_rules! impl_rx_delegate {
    ($target:ident, $is_const:expr) => {
        impl<T: $crate::traits::RxData> $crate::traits::RxValue for $target<T> {
            type Value = T;
        }

        impl<T: $crate::traits::RxData> $crate::traits::RxBase for $target<T> {
            fn raw_id(&self) -> Option<$crate::reactivity::RawId> {
                None
            }
            fn track(&self) {}
            fn is_disposed(&self) -> bool {
                false
            }
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                None
            }
            fn debug_name(&self) -> Option<String> {
                None
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::IntoRx for $target<T> {
            type RxType = $crate::Rx<T, $crate::RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                $crate::Rx::derive(Box::new(move || {
                    use $crate::traits::RxGet;
                    self.get()
                }))
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                $is_const
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::IntoSignal for $target<T> {
            #[inline(always)]
            fn into_signal(self) -> $crate::reactivity::Signal<T> {
                $crate::reactivity::Signal::derive(Box::new(move || {
                    $crate::traits::RxRead::get(&self)
                }))
            }
        }
    };
    ($target:ident, SignalID, $is_const:expr) => {
        impl<T: $crate::traits::RxData> $crate::traits::RxValue for $target<T> {
            type Value = T;
        }

        impl<T: $crate::traits::RxData> $crate::traits::RxBase for $target<T> {
            #[inline(always)]
            fn raw_id(&self) -> Option<$crate::reactivity::RawId> {
                Some(::silex_reactivity::AnyHandle::into_raw(self.id))
            }
            #[inline(always)]
            fn track(&self) {
                let id = ::silex_reactivity::AnyHandle::into_raw(self.id);
                ::silex_reactivity::signal::track(
                    ::silex_reactivity::SignalId::from_raw_unchecked(id),
                );
            }
            #[inline(always)]
            fn is_disposed(&self) -> bool {
                let id = ::silex_reactivity::AnyHandle::into_raw(self.id);
                !::silex_reactivity::SignalId::from_raw_unchecked(id).is_alive()
            }
            #[inline(always)]
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                ::silex_reactivity::get_node_defined_at(self.id)
            }
            #[inline(always)]
            fn debug_name(&self) -> Option<String> {
                ::silex_reactivity::get_debug_label(self.id)
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::IntoRx for $target<T> {
            type RxType = $crate::Rx<T, $crate::RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                let id = ::silex_reactivity::AnyHandle::into_raw(self.id);
                $crate::Rx::new_signal(id)
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                $is_const
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::IntoSignal for $target<T> {
            #[inline(always)]
            fn into_signal(self) -> $crate::reactivity::Signal<T> {
                let id = ::silex_reactivity::AnyHandle::into_raw(self.id);
                $crate::reactivity::Signal::Read($crate::reactivity::ReadSignal {
                    id,
                    marker: ::core::marker::PhantomData,
                })
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::RxInternal for $target<T> {
            type ReadOutput<'a>
                = $crate::traits::RxGuard<'a, T, T>
            where
                Self: 'a;

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                let id = ::silex_reactivity::AnyHandle::into_raw(self.id);
                // SAFETY: 借用被立刻收窄回 `RxGuard<'_, T, T>`，其生命周期挂在
                // `&self` 上，不会逃逸出调用方的表达式作用域。
                //
                // 残留风险（AUDIT P6 未闭环的部分）：句柄是 `Copy` 的，它的存活与
                // 节点的存活无关 —— 调用方若在持有 guard 期间 `dispose` 这个节点，
                // 仍会读到已释放的内存。彻底修复需要运行时级别的借用计数。
                let val = unsafe {
                    ::silex_reactivity::signal::try_value_ref::<T>(
                        ::silex_reactivity::SignalId::from_raw_unchecked(id),
                    )?
                };
                Some($crate::traits::RxGuard::Borrowed {
                    value: val,
                    token: Some(id),
                })
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                let id = ::silex_reactivity::AnyHandle::into_raw(self.id);
                ::silex_reactivity::signal::try_with_untracked(
                    ::silex_reactivity::SignalId::from_raw_unchecked(id),
                    fun,
                )
                .ok()
            }

            #[inline(always)]
            fn rx_get_adaptive(&self) -> Option<Self::Value> {
                self.rx_try_with_untracked(|v| {
                    use $crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
                    AdaptiveWrapper(v).maybe_clone()
                })
                .flatten()
            }

            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                $is_const
            }
        }
    };
    ($target:ident, $field:ident, $is_const:expr) => {
        impl<T: $crate::traits::RxData> $crate::traits::RxValue for $target<T> {
            type Value = T;
        }

        impl<T: $crate::traits::RxData> $crate::traits::RxBase for $target<T> {
            #[inline(always)]
            fn raw_id(&self) -> Option<$crate::reactivity::RawId> {
                $crate::traits::RxBase::raw_id(&self.$field)
            }
            #[inline(always)]
            fn track(&self) {
                $crate::traits::RxBase::track(&self.$field)
            }
            #[inline(always)]
            fn is_disposed(&self) -> bool {
                $crate::traits::RxBase::is_disposed(&self.$field)
            }
            #[inline(always)]
            fn defined_at(&self) -> Option<&'static ::std::panic::Location<'static>> {
                $crate::traits::RxBase::defined_at(&self.$field)
            }
            #[inline(always)]
            fn debug_name(&self) -> Option<String> {
                $crate::traits::RxBase::debug_name(&self.$field)
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::IntoRx for $target<T> {
            type RxType = $crate::Rx<T, $crate::RxValueKind>;
            #[inline(always)]
            fn into_rx(self) -> Self::RxType {
                $crate::traits::IntoRx::into_rx(self.$field)
            }
            #[inline(always)]
            fn is_constant(&self) -> bool {
                $is_const
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::IntoSignal for $target<T> {
            #[inline(always)]
            fn into_signal(self) -> $crate::reactivity::Signal<T> {
                $crate::traits::IntoSignal::into_signal(self.$field)
            }
        }

        impl<T: $crate::traits::RxData> $crate::traits::RxInternal for $target<T> {
            type ReadOutput<'a>
                = <$crate::reactivity::ReadSignal<T> as $crate::traits::RxInternal>::ReadOutput<'a>
            where
                Self: 'a;

            #[inline(always)]
            fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
                self.$field.rx_read_untracked()
            }

            #[inline(always)]
            fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
                self.$field.rx_try_with_untracked(fun)
            }

            #[inline(always)]
            fn rx_get_adaptive(&self) -> Option<Self::Value> {
                self.$field.rx_get_adaptive()
            }

            #[inline(always)]
            fn rx_is_constant(&self) -> bool {
                $is_const
            }
        }
    };
}
