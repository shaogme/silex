use std::marker::PhantomData;
use std::panic::Location;

use silex_reactivity::NodeId;

use crate::traits::*;
use crate::{Rx, RxValue};

// --- StoredValue ---

pub struct StoredValue<T> {
    pub(crate) id: NodeId,
    pub(crate) marker: PhantomData<T>,
}

impl<T> std::fmt::Debug for StoredValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StoredValue({:?})", self.id)
    }
}

impl<T> Clone for StoredValue<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for StoredValue<T> {}

impl<T: 'static> StoredValue<T> {
    pub fn new(value: T) -> Self {
        let id = silex_reactivity::store_value(value);
        Self {
            id,
            marker: PhantomData,
        }
    }

    // Kept for backward compat or ease of use
    // Kept for backward compat or ease of use
    pub fn set_untracked(&self, value: T) {
        SetUntracked::set_untracked(self, value)
    }

    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        RxRead::get_untracked(self)
    }

    pub fn with_name(self, name: impl Into<String>) -> Self {
        silex_reactivity::set_debug_label(self.id, name);
        self
    }
}

// Note: GetUntracked is now blanket-implemented via WithUntracked when T: Clone

impl<T: 'static> RxBase for StoredValue<T> {
    type Value = T;

    #[inline(always)]
    fn id(&self) -> Option<NodeId> {
        Some(self.id)
    }

    #[inline(always)]
    fn track(&self) {
        // StoredValue is non-reactive, no-op
    }

    #[inline(always)]
    fn is_disposed(&self) -> bool {
        !silex_reactivity::is_stored_value_valid(self.id)
    }

    #[inline(always)]
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        None
    }

    #[inline(always)]
    fn debug_name(&self) -> Option<String> {
        silex_reactivity::get_debug_label(self.id)
    }
}

impl<T: 'static> RxInternal for StoredValue<T> {
    type ReadOutput<'a>
        = RxGuard<'a, T, T>
    where
        Self: 'a;

    #[inline(always)]
    fn rx_read_untracked(&self) -> Option<Self::ReadOutput<'_>> {
        unsafe {
            silex_reactivity::try_with_stored_value(self.id, |v: &T| {
                std::mem::transmute::<&T, &'static T>(v)
            })
            .map(|v| RxGuard::Borrowed {
                value: v,
                token: Some(crate::NodeRef::from_id(self.id)),
            })
        }
    }

    #[inline(always)]
    fn rx_try_with_untracked<U>(&self, fun: impl FnOnce(&Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_with_stored_value(self.id, fun)
    }

    #[inline(always)]
    fn rx_get_adaptive(&self) -> Option<Self::Value>
    where
        Self::Value: Sized,
    {
        self.rx_try_with_untracked(|v| {
            use crate::traits::adaptive::{AdaptiveFallback, AdaptiveWrapper};
            AdaptiveWrapper(v).maybe_clone()
        })
        .flatten()
    }

    #[inline(always)]
    fn rx_is_constant(&self) -> bool {
        true
    }
}

impl<T: 'static> IntoRx for StoredValue<T> {
    type Value = T;
    type RxType = Rx<Self, RxValue>;
    #[inline(always)]
    fn into_rx(self) -> Self::RxType {
        Rx(self, PhantomData)
    }
    #[inline(always)]
    fn is_constant(&self) -> bool {
        true
    }
    #[inline(always)]
    fn into_signal(self) -> crate::reactivity::Signal<T> {
        crate::reactivity::Signal::StoredConstant(self.id, PhantomData)
    }
}

impl<T: 'static> UpdateUntracked for StoredValue<T> {
    fn try_update_untracked<U>(&self, fun: impl FnOnce(&mut Self::Value) -> U) -> Option<U> {
        silex_reactivity::try_update_stored_value(self.id, fun)
    }
}
